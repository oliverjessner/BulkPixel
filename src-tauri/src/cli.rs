use std::{collections::HashSet, env, path::Path, time::Instant};

use crate::{
    image_pipeline::convert_images,
    models::{
        CollisionMode, ConversionImageInput, ConversionPreset, ConversionRequest,
        ConversionResponse, ConversionStatistics, ExportFormat, ResizeOptions, SavePresetRequest,
    },
    presets::{
        delete_preset_by_name_for_cli, find_preset_by_name_for_cli, list_presets_for_cli,
        load_statistics_for_cli, record_conversion_statistics_for_cli, save_preset_for_cli,
    },
};

const HELP_TEXT: &str = "\
BulkPixel CLI

Usage:
  bulkpixel convert --input <file...> --output-dir <dir> [options]
  bulkpixel convert --preset <name...> --input <file...> [--silent]
  bulkpixel presets list
  bulkpixel presets create --name <name> --output-dir <dir> --format <format> [options]
  bulkpixel presets update --name <name> [options]
  bulkpixel presets delete --name <name>
  bulkpixel stats
  bulkpixel --help
  bulkpixel --version

Formats:
  jpeg, png, webp, avif
";

#[derive(Debug, Default)]
struct ConvertOptions {
    inputs: Vec<String>,
    preset_names: Vec<String>,
    output_dir: Option<String>,
    width: Option<String>,
    height: Option<String>,
    format: Option<String>,
    quality: Option<String>,
    prefix: Option<String>,
    postfix: Option<String>,
    overwrite: bool,
    silent: bool,
}

#[derive(Debug, Default)]
struct PresetOptions {
    name: Option<String>,
    output_dir: Option<String>,
    width: Option<String>,
    height: Option<String>,
    format: Option<String>,
    quality: Option<String>,
    prefix: Option<String>,
    postfix: Option<String>,
}

#[derive(Debug)]
struct ConversionReport {
    label: Option<String>,
    input_format: String,
    output_format: String,
    output_folder: String,
    response: ConversionResponse,
}

pub fn run_from_env() -> i32 {
    let args = env::args().skip(1).collect::<Vec<_>>();

    match execute(args) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("Error: {error}");
            1
        }
    }
}

fn execute(args: Vec<String>) -> Result<i32, String> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("{HELP_TEXT}");
        return Ok(0);
    }

    if args[0] == "--version" || args[0] == "-V" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }

    match args[0].as_str() {
        "convert" => handle_convert(&args[1..]),
        "presets" => handle_presets(&args[1..]),
        "stats" => handle_stats(&args[1..]),
        command => Err(format!("Unknown command: {command}")),
    }
}

fn handle_convert(args: &[String]) -> Result<i32, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP_TEXT}");
        return Ok(0);
    }

    let options = parse_convert_options(args)?;

    if options.preset_names.is_empty() {
        run_direct_conversion(options)
    } else {
        run_preset_conversion(options)
    }
}

fn run_direct_conversion(options: ConvertOptions) -> Result<i32, String> {
    let inputs = require_inputs(&options.inputs)?;
    let output_dir = options
        .output_dir
        .clone()
        .ok_or_else(|| "--output-dir is required.".to_string())?;
    let format = parse_optional_format(options.format.as_deref())?.unwrap_or(ExportFormat::Webp);
    let quality = parse_optional_quality(options.quality.as_deref())?.unwrap_or(100);
    let resize = parse_resize(options.width.as_deref(), options.height.as_deref())?;
    let (filename_mode, filename_component) =
        parse_filename_options(options.prefix.as_deref(), options.postfix.as_deref())?;
    let input_format = input_format_label(&inputs);
    let output_format = format.label().to_string();
    let collision_mode = if options.overwrite {
        CollisionMode::Overwrite
    } else {
        CollisionMode::Error
    };

    let request = ConversionRequest {
        images: conversion_inputs(inputs),
        format: format.clone(),
        resize,
        quality,
        filename_component,
        filename_mode,
        output_dir: output_dir.clone(),
        collision_mode,
    };

    let started_at = Instant::now();
    let response = convert_images(request).map_err(|error| error.to_string())?;
    record_statistics(&format, &response, started_at, options.silent);
    let has_failures = response.summary.failure_count > 0;
    let report = ConversionReport {
        label: None,
        input_format,
        output_format,
        output_folder: output_dir,
        response,
    };

    if !options.silent {
        print_single_conversion_report(&report);
    }

    Ok(if has_failures { 1 } else { 0 })
}

fn run_preset_conversion(options: ConvertOptions) -> Result<i32, String> {
    let inputs = require_inputs(&options.inputs)?;
    let presets = load_requested_presets(&options.preset_names)?;

    if presets.len() > 1 {
        validate_multiple_preset_markers(&presets)?;
    }

    let mut reports = Vec::with_capacity(presets.len());

    for preset in presets {
        let format = parse_format(&preset.format)?;
        let resize = resize_from_preset(&preset)?;
        let input_format = input_format_label(&inputs);
        let output_format = format.label().to_string();
        let request = ConversionRequest {
            images: conversion_inputs(inputs.clone()),
            format: format.clone(),
            resize,
            quality: preset.quality,
            filename_component: preset.filename_component.clone(),
            filename_mode: preset.filename_mode.clone(),
            output_dir: preset.output_directory.clone(),
            collision_mode: CollisionMode::Error,
        };

        let started_at = Instant::now();
        let response = convert_images(request).map_err(|error| error.to_string())?;
        record_statistics(&format, &response, started_at, options.silent);

        reports.push(ConversionReport {
            label: Some(preset.name),
            input_format,
            output_format,
            output_folder: preset.output_directory,
            response,
        });
    }

    let has_failures = reports
        .iter()
        .any(|report| report.response.summary.failure_count > 0);

    if !options.silent {
        if reports.len() == 1 {
            print_single_conversion_report(&reports[0]);
        } else {
            print_multi_preset_report(&reports);
        }
    }

    Ok(if has_failures { 1 } else { 0 })
}

fn handle_presets(args: &[String]) -> Result<i32, String> {
    let Some(command) = args.first() else {
        return Err("Missing presets command.".into());
    };

    if command == "--help" || command == "-h" {
        println!("{HELP_TEXT}");
        return Ok(0);
    }

    match command.as_str() {
        "list" => {
            let presets = list_presets_for_cli().map_err(|error| error.to_string())?;
            print_presets(&presets);
            Ok(0)
        }
        "create" => {
            let options = parse_preset_options(&args[1..])?;
            let request = build_create_preset_request(options)?;
            let preset = save_preset_for_cli(request).map_err(|error| error.to_string())?;
            println!("Preset created: {}", preset.name);
            Ok(0)
        }
        "update" => {
            let options = parse_preset_options(&args[1..])?;
            let preset = update_preset_request(options)?;
            let preset = save_preset_for_cli(preset).map_err(|error| error.to_string())?;
            println!("Preset updated: {}", preset.name);
            Ok(0)
        }
        "delete" => {
            let options = parse_preset_options(&args[1..])?;
            let name = options
                .name
                .ok_or_else(|| "--name is required.".to_string())?;
            delete_preset_by_name_for_cli(&name).map_err(|error| error.to_string())?;
            println!("Preset deleted: {name}");
            Ok(0)
        }
        other => Err(format!("Unknown presets command: {other}")),
    }
}

fn handle_stats(args: &[String]) -> Result<i32, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP_TEXT}");
        return Ok(0);
    }

    let statistics = load_statistics_for_cli().map_err(|error| error.to_string())?;
    print_statistics(&statistics);
    Ok(0)
}

fn parse_convert_options(args: &[String]) -> Result<ConvertOptions, String> {
    let mut options = ConvertOptions {
        format: Some("webp".into()),
        quality: Some("100".into()),
        ..Default::default()
    };
    let mut index = 0_usize;

    while index < args.len() {
        match args[index].as_str() {
            "--input" => {
                let (values, next_index) = take_values(args, index)?;
                options.inputs = values;
                index = next_index;
            }
            "--preset" => {
                let (values, next_index) = take_values(args, index)?;
                options.preset_names = values;
                index = next_index;
            }
            "--output-dir" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.output_dir = Some(value);
                index = next_index;
            }
            "--width" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.width = Some(value);
                index = next_index;
            }
            "--height" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.height = Some(value);
                index = next_index;
            }
            "--format" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.format = Some(value);
                index = next_index;
            }
            "--quality" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.quality = Some(value);
                index = next_index;
            }
            "--prefix" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.prefix = Some(value);
                index = next_index;
            }
            "--postfix" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.postfix = Some(value);
                index = next_index;
            }
            "--overwrite" => {
                options.overwrite = true;
                index += 1;
            }
            "--silent" => {
                options.silent = true;
                index += 1;
            }
            flag => return Err(format!("Unknown convert flag: {flag}")),
        }
    }

    Ok(options)
}

fn parse_preset_options(args: &[String]) -> Result<PresetOptions, String> {
    let mut options = PresetOptions::default();
    let mut index = 0_usize;

    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.name = Some(value);
                index = next_index;
            }
            "--output-dir" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.output_dir = Some(value);
                index = next_index;
            }
            "--width" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.width = Some(value);
                index = next_index;
            }
            "--height" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.height = Some(value);
                index = next_index;
            }
            "--format" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.format = Some(value);
                index = next_index;
            }
            "--quality" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.quality = Some(value);
                index = next_index;
            }
            "--prefix" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.prefix = Some(value);
                index = next_index;
            }
            "--postfix" => {
                let (value, next_index) = take_one_value(args, index)?;
                options.postfix = Some(value);
                index = next_index;
            }
            flag => return Err(format!("Unknown presets flag: {flag}")),
        }
    }

    Ok(options)
}

fn take_values(args: &[String], flag_index: usize) -> Result<(Vec<String>, usize), String> {
    let flag = &args[flag_index];
    let mut index = flag_index + 1;
    let mut values = Vec::new();

    while index < args.len() && !args[index].starts_with("--") {
        values.push(args[index].clone());
        index += 1;
    }

    if values.is_empty() {
        return Err(format!("{flag} expects at least one value."));
    }

    Ok((values, index))
}

fn take_one_value(args: &[String], flag_index: usize) -> Result<(String, usize), String> {
    let flag = &args[flag_index];
    let (values, next_index) = take_values(args, flag_index)?;

    if values.len() != 1 {
        return Err(format!("{flag} expects exactly one value."));
    }

    Ok((values[0].clone(), next_index))
}

fn build_create_preset_request(options: PresetOptions) -> Result<SavePresetRequest, String> {
    let name = options
        .name
        .ok_or_else(|| "--name is required.".to_string())?;
    let output_directory = options
        .output_dir
        .ok_or_else(|| "--output-dir is required.".to_string())?;
    let format = options
        .format
        .as_deref()
        .ok_or_else(|| "--format is required.".to_string())
        .and_then(parse_format)?;
    let quality = parse_optional_quality(options.quality.as_deref())?.unwrap_or(100);
    let (resize_mode, width, height) =
        preset_resize_options(options.width.as_deref(), options.height.as_deref())?;
    let (filename_mode, filename_component) =
        parse_filename_options(options.prefix.as_deref(), options.postfix.as_deref())?;

    Ok(SavePresetRequest {
        id: None,
        name,
        format: format_to_preset_value(&format),
        resize_mode,
        width,
        height,
        quality,
        filename_component,
        filename_mode,
        output_directory,
    })
}

fn update_preset_request(options: PresetOptions) -> Result<SavePresetRequest, String> {
    let name = options
        .name
        .clone()
        .ok_or_else(|| "--name is required.".to_string())?;
    let existing = find_preset_by_name_for_cli(&name).map_err(|error| error.to_string())?;
    let mut request = SavePresetRequest {
        id: Some(existing.id),
        name: existing.name,
        format: existing.format,
        resize_mode: existing.resize_mode,
        width: existing.width,
        height: existing.height,
        quality: existing.quality,
        filename_component: existing.filename_component,
        filename_mode: existing.filename_mode,
        output_directory: existing.output_directory,
    };

    if let Some(format) = options.format.as_deref() {
        request.format = format_to_preset_value(&parse_format(format)?);
    }

    if options.width.is_some() || options.height.is_some() {
        let (resize_mode, width, height) =
            preset_resize_options(options.width.as_deref(), options.height.as_deref())?;
        request.resize_mode = resize_mode;
        request.width = width;
        request.height = height;
    }

    if let Some(quality) = parse_optional_quality(options.quality.as_deref())? {
        request.quality = quality;
    }

    if let Some(output_directory) = options.output_dir {
        request.output_directory = output_directory;
    }

    if options.prefix.is_some() || options.postfix.is_some() {
        let (filename_mode, filename_component) =
            parse_filename_options(options.prefix.as_deref(), options.postfix.as_deref())?;
        request.filename_mode = filename_mode;
        request.filename_component = filename_component;
    }

    Ok(request)
}

fn require_inputs(inputs: &[String]) -> Result<Vec<String>, String> {
    if inputs.is_empty() {
        return Err("--input is required.".into());
    }

    Ok(inputs.to_vec())
}

fn conversion_inputs(inputs: Vec<String>) -> Vec<ConversionImageInput> {
    inputs
        .into_iter()
        .map(|path| ConversionImageInput { path })
        .collect()
}

fn parse_optional_format(value: Option<&str>) -> Result<Option<ExportFormat>, String> {
    value.map(parse_format).transpose()
}

fn parse_format(value: &str) -> Result<ExportFormat, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => Ok(ExportFormat::Jpeg),
        "png" => Ok(ExportFormat::Png),
        "webp" => Ok(ExportFormat::Webp),
        "avif" => Ok(ExportFormat::Avif),
        _ => Err(format!("Unsupported format: {value}")),
    }
}

fn format_to_preset_value(format: &ExportFormat) -> String {
    match format {
        ExportFormat::Jpeg => "jpeg",
        ExportFormat::Png => "png",
        ExportFormat::Webp => "webp",
        ExportFormat::Avif => "avif",
    }
    .into()
}

fn parse_optional_quality(value: Option<&str>) -> Result<Option<u8>, String> {
    value.map(parse_quality).transpose()
}

fn parse_quality(value: &str) -> Result<u8, String> {
    let quality = value
        .parse::<u8>()
        .map_err(|_| "Quality must be a number between 1 and 100.".to_string())?;

    if !(1..=100).contains(&quality) {
        return Err("Quality must be between 1 and 100.".into());
    }

    Ok(quality)
}

fn parse_resize(width: Option<&str>, height: Option<&str>) -> Result<ResizeOptions, String> {
    if width.is_some() && height.is_some() {
        return Err("Use either --width or --height, not both.".into());
    }

    Ok(ResizeOptions {
        width: width.map(parse_dimension).transpose()?,
        height: height.map(parse_dimension).transpose()?,
    })
}

fn preset_resize_options(
    width: Option<&str>,
    height: Option<&str>,
) -> Result<(String, Option<u32>, Option<u32>), String> {
    let resize = parse_resize(width, height)?;

    match (resize.width, resize.height) {
        (Some(width), None) => Ok(("width".into(), Some(width), None)),
        (None, Some(height)) => Ok(("height".into(), None, Some(height))),
        _ => Ok(("none".into(), None, None)),
    }
}

fn parse_dimension(value: &str) -> Result<u32, String> {
    let dimension = value
        .parse::<u32>()
        .map_err(|_| "Width and height must be numbers between 1 and 9999.".to_string())?;

    if !(1..=9999).contains(&dimension) {
        return Err("Width and height must be between 1 and 9999.".into());
    }

    Ok(dimension)
}

fn parse_filename_options(
    prefix: Option<&str>,
    postfix: Option<&str>,
) -> Result<(String, String), String> {
    let prefix = prefix.map(str::trim).filter(|value| !value.is_empty());
    let postfix = postfix.map(str::trim).filter(|value| !value.is_empty());

    match (prefix, postfix) {
        (Some(_), Some(_)) => Err("Use either --prefix or --postfix, not both.".into()),
        (Some(prefix), None) => Ok(("prefix".into(), prefix.into())),
        (None, Some(postfix)) => Ok(("postfix".into(), postfix.into())),
        (None, None) => Ok(("prefix".into(), String::new())),
    }
}

fn resize_from_preset(preset: &ConversionPreset) -> Result<ResizeOptions, String> {
    match preset.resize_mode.as_str() {
        "width" => Ok(ResizeOptions {
            width: preset.width,
            height: None,
        }),
        "height" => Ok(ResizeOptions {
            width: None,
            height: preset.height,
        }),
        "none" => Ok(ResizeOptions {
            width: None,
            height: None,
        }),
        mode => Err(format!("Unsupported preset resize mode: {mode}")),
    }
}

fn load_requested_presets(names: &[String]) -> Result<Vec<ConversionPreset>, String> {
    if names.is_empty() {
        return Err("--preset expects at least one preset name.".into());
    }

    names
        .iter()
        .map(|name| find_preset_by_name_for_cli(name).map_err(|error| error.to_string()))
        .collect()
}

fn validate_multiple_preset_markers(presets: &[ConversionPreset]) -> Result<(), String> {
    let mut markers = HashSet::new();

    for preset in presets {
        let component = preset.filename_component.trim();
        if component.is_empty() {
            return Err(format!(
                "Preset collision risk: '{}' has no prefix or postfix.",
                preset.name
            ));
        }

        let marker = format!("{}:{component}", preset.filename_mode);
        if !markers.insert(marker) {
            return Err(
                "Preset collision risk: multiple presets use the same prefix/postfix.".into(),
            );
        }
    }

    Ok(())
}

fn record_statistics(
    format: &ExportFormat,
    response: &ConversionResponse,
    started_at: Instant,
    silent: bool,
) {
    if let Err(error) = record_conversion_statistics_for_cli(
        format,
        &response.summary,
        started_at.elapsed().as_millis(),
    ) {
        if !silent {
            eprintln!("Warning: failed to update statistics: {error}");
        }
    }
}

fn input_format_label(inputs: &[String]) -> String {
    let mut labels = HashSet::new();

    for input in inputs {
        labels.insert(format_label_from_path(input));
    }

    if labels.len() == 1 {
        labels.into_iter().next().unwrap_or_else(|| "IMAGE".into())
    } else {
        "MIXED".into()
    }
}

fn format_label_from_path(path: &str) -> String {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "JPEG".into(),
        Some("png") => "PNG".into(),
        Some("webp") => "WEBP".into(),
        Some("avif") => "AVIF".into(),
        _ => "IMAGE".into(),
    }
}

fn print_single_conversion_report(report: &ConversionReport) {
    println!("BulkPixel Conversion Complete");
    println!("-----------------------------");

    if let Some(label) = &report.label {
        println!("Preset: {label}");
    }

    println!(
        "Images: {}/{} converted",
        report.response.summary.success_count,
        report.response.results.len()
    );
    println!(
        "Format: {} {} {}",
        report.input_format,
        format_arrow(),
        report.output_format
    );
    println!(
        "Input: {}",
        format_bytes(report.response.summary.total_original_size)
    );
    println!(
        "Output: {}",
        format_bytes(report.response.summary.total_converted_size)
    );
    println!("Saved: {}", format_bytes(saved_bytes(&report.response)));
    println!("Output Folder: {}", report.output_folder);
    print_failure_messages(&report.response);
}

fn print_multi_preset_report(reports: &[ConversionReport]) {
    println!("BulkPixel Conversion Complete");
    println!("-----------------------------");
    println!(
        "Presets: {}",
        reports
            .iter()
            .filter_map(|report| report.label.as_deref())
            .collect::<Vec<_>>()
            .join(", ")
    );

    for report in reports {
        println!();
        if let Some(label) = &report.label {
            println!("{label}");
        }
        println!(
            "Images: {}/{} converted",
            report.response.summary.success_count,
            report.response.results.len()
        );
        println!(
            "Format: {} {} {}",
            report.input_format,
            format_arrow(),
            report.output_format
        );
        println!("Saved: {}", format_bytes(saved_bytes(&report.response)));
        print_failure_messages(&report.response);
    }

    let success_count = reports
        .iter()
        .map(|report| report.response.summary.success_count)
        .sum::<usize>();
    let total_count = reports
        .iter()
        .map(|report| report.response.results.len())
        .sum::<usize>();
    let input_bytes = reports
        .iter()
        .map(|report| report.response.summary.total_original_size)
        .sum::<u64>();
    let output_bytes = reports
        .iter()
        .map(|report| report.response.summary.total_converted_size)
        .sum::<u64>();
    let saved = input_bytes.saturating_sub(output_bytes);

    println!();
    println!("Total");
    println!("Images: {success_count}/{total_count} converted");
    println!("Input: {}", format_bytes(input_bytes));
    println!("Output: {}", format_bytes(output_bytes));
    println!("Saved: {}", format_bytes(saved));
}

fn print_presets(presets: &[ConversionPreset]) {
    println!("BulkPixel Presets ({})", presets.len());
    println!("-----------------");

    for (index, preset) in presets.iter().enumerate() {
        if index > 0 {
            println!();
        }

        println!("Name: {}", preset.name);
        println!("Export Format: {}", preset.format.to_ascii_uppercase());
        println!(
            "Width: {}",
            preset_dimension_label(preset.width, preset.resize_mode == "none")
        );
        println!(
            "Height: {}",
            preset_dimension_label(preset.height, preset.resize_mode == "none")
        );
        println!("Quality: {}%", preset.quality);

        if !preset.filename_component.trim().is_empty() {
            if preset.filename_mode == "postfix" {
                println!("Postfix: {}", preset.filename_component);
            } else {
                println!("Prefix: {}", preset.filename_component);
            }
        }

        println!("Output Folder: {}", preset.output_directory);
    }
}

fn preset_dimension_label(value: Option<u32>, is_original: bool) -> String {
    match (value, is_original) {
        (Some(value), _) => value.to_string(),
        (None, true) => "Original".into(),
        (None, false) => "Auto".into(),
    }
}

fn print_statistics(statistics: &ConversionStatistics) {
    println!("BulkPixel Statistics");
    println!("--------------------");
    println!("Conversions");
    println!("Total: {}", statistics.amount);
    println!("WEBP: {}", statistics.webp);
    println!("PNG: {}", statistics.png);
    println!("AVIF: {}", statistics.avif);
    println!("JPEG: {}", statistics.jpeg);
    println!();
    println!("Storage");
    println!("Input: {}", format_bytes_i64(statistics.input_bytes));
    println!("Output: {}", format_bytes_i64(statistics.output_bytes));
    println!("Saved: {}", format_bytes_i64(statistics.saved_bytes));
    println!();
    println!("Performance");
    println!(
        "Processing Time: {}",
        format_duration(statistics.processing_time_ms)
    );
    println!();
    println!("Timeline");
    println!("First Conversion: {}", format_date(&statistics.created_at));
    println!(
        "Last Conversion: {}",
        format_date(&statistics.last_conversion_at)
    );
}

fn print_failure_messages(response: &ConversionResponse) {
    let failures = response
        .results
        .iter()
        .filter(|result| !result.success)
        .collect::<Vec<_>>();

    if failures.is_empty() {
        return;
    }

    println!("Failures:");
    for failure in failures {
        println!("- {}: {}", failure.original_name, failure.message);
    }
}

fn saved_bytes(response: &ConversionResponse) -> u64 {
    response
        .summary
        .total_original_size
        .saturating_sub(response.summary.total_converted_size)
}

fn format_bytes_i64(bytes: i64) -> String {
    format_bytes(u64::try_from(bytes).unwrap_or_default())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_index = 0_usize;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

fn format_duration(milliseconds: i64) -> String {
    let seconds = (milliseconds / 1000).max(0);
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;

    if minutes > 0 {
        format!("{minutes}min {remaining_seconds}sec")
    } else {
        format!("{remaining_seconds}sec")
    }
}

fn format_date(value: &str) -> String {
    if value.len() >= 10 {
        let year = &value[0..4];
        let month = &value[5..7];
        let day = &value[8..10];
        format!("{day}.{month}.{year}")
    } else {
        value.into()
    }
}

fn format_arrow() -> &'static str {
    "\u{2192}"
}
