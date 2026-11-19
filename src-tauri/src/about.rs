//! The macOS About panel uses AppKit controls and inherits the system appearance.

use std::cell::OnceCell;

use objc2::{
    define_class, msg_send, rc::Retained, runtime::AnyObject, sel, AnyThread, MainThreadOnly,
};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSFont, NSImage, NSImageScaling, NSImageView,
    NSLayoutAttribute, NSPanel, NSStackView, NSStackViewDistribution, NSTextAlignment, NSTextField,
    NSUserInterfaceLayoutOrientation, NSWindowStyleMask, NSWindowTitleVisibility, NSWorkspace,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSData, NSEdgeInsets, NSObject, NSObjectProtocol, NSPoint, NSRect,
    NSString, NSURL,
};
use tauri::menu::{Menu, MenuItem};

const MENU_ID: &str = "bulkpixel.about";

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this type has no Drop implementation.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct AboutLinkTarget;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for AboutLinkTarget {}

    impl AboutLinkTarget {
        // SAFETY: This is a standard target/action method receiving the sender object.
        #[unsafe(method(openWebsite:))]
        fn open_website(&self, _sender: Option<&AnyObject>) {
            if let Some(url) = NSURL::URLWithString(ns_string!("https://oliverjessner.at")) {
                if !NSWorkspace::sharedWorkspace().openURL(&url) {
                    eprintln!("failed to open the About website in the default browser");
                }
            }
        }
    }
);

struct AboutPanel {
    panel: Retained<NSPanel>,
    // AppKit's target/action controls do not retain their targets.
    _link_target: Retained<AboutLinkTarget>,
}

thread_local! {
    static ABOUT_PANEL: OnceCell<AboutPanel> = const { OnceCell::new() };
}

pub(crate) fn install(
    app: &tauri::AppHandle,
    menu: &Menu<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Menu::default places About first in the macOS application submenu.
    // Keep every other standard menu item and its native behavior intact.
    let items = menu.items()?;
    let app_menu = items
        .first()
        .and_then(|item| item.as_submenu())
        .ok_or("The macOS application menu is unavailable.")?;
    let name = app_name(app);
    let item = MenuItem::with_id(app, MENU_ID, format!("About {name}"), true, None::<&str>)?;
    app_menu.remove_at(0)?;
    app_menu.insert(&item, 0)?;

    app.on_menu_event(|app, event| {
        if event.id().as_ref() == MENU_ID {
            let handle = app.clone();
            if let Err(error) = app.run_on_main_thread(move || {
                if let Err(error) = show(&handle) {
                    eprintln!("failed to show the About panel: {error}");
                }
            }) {
                eprintln!("failed to schedule the About panel: {error}");
            }
        }
    });
    Ok(())
}

fn app_name(app: &tauri::AppHandle) -> &str {
    app.config()
        .product_name
        .as_deref()
        .unwrap_or(&app.package_info().name)
}

fn show(app: &tauri::AppHandle) -> Result<(), &'static str> {
    let mtm = MainThreadMarker::new().ok_or("The About panel requires the main thread.")?;
    ABOUT_PANEL.with(|state| {
        if state.get().is_none() {
            state
                .set(create_panel(app, mtm)?)
                .map_err(|_| "The About panel is already initialized.")?;
        }
        if let Some(about) = state.get() {
            about.panel.makeKeyAndOrderFront(None);
        }
        Ok(())
    })
}

fn create_panel(app: &tauri::AppHandle, mtm: MainThreadMarker) -> Result<AboutPanel, &'static str> {
    // Embed an existing configured app icon, including when running outside the .app bundle.
    let icon_data = NSData::with_bytes(include_bytes!("../icons/128x128@2x.png"));
    let image = NSImage::initWithData(NSImage::alloc(), &icon_data)
        .ok_or("The app icon could not be loaded.")?;
    let icon = NSImageView::new(mtm);
    icon.setImage(Some(&image));
    icon.setImageScaling(NSImageScaling::ProportionallyUpOrDown);
    icon.setTranslatesAutoresizingMaskIntoConstraints(false);
    // AppKit uses logical points, so the icon stays sharp at 52 × 52 on Retina displays.
    icon.widthAnchor()
        .constraintEqualToConstant(52.0)
        .setActive(true);
    icon.heightAnchor()
        .constraintEqualToConstant(52.0)
        .setActive(true);

    let body_font = NSFont::systemFontOfSize(12.0);
    let secondary = NSColor::secondaryLabelColor();
    let name = label(
        app_name(app),
        &NSFont::boldSystemFontOfSize(17.0),
        &NSColor::labelColor(),
        mtm,
    );
    let version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| app.package_info().version.to_string());
    let version = label(&format!("Version {version}"), &body_font, &secondary, mtm);
    let copyright = label("Copyright © 2026 Oliver Jessner", &body_font, &secondary, mtm);
    let license = label("MIT License", &body_font, &secondary, mtm);

    // SAFETY: This is NSObject's standard initializer on our main-thread-only subclass.
    let link_target: Retained<AboutLinkTarget> =
        unsafe { msg_send![super(AboutLinkTarget::alloc(mtm)), init] };
    // SAFETY: The retained target implements openWebsite: with the standard action signature.
    let website = unsafe {
        NSButton::buttonWithTitle_target_action(
            ns_string!("oliverjessner.at"),
            Some(&link_target),
            Some(sel!(openWebsite:)),
            mtm,
        )
    };
    website.setBordered(false);
    website.setFont(Some(&body_font));
    website.setContentTintColor(Some(&NSColor::linkColor()));
    website.setTranslatesAutoresizingMaskIntoConstraints(false);
    website.sizeToFit();

    let content = NSStackView::new(mtm);
    content.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
    content.setAlignment(NSLayoutAttribute::CenterX);
    content.setDistribution(NSStackViewDistribution::Fill);
    content.setSpacing(8.0);
    content.setEdgeInsets(NSEdgeInsets {
        top: 24.0,
        left: 32.0,
        bottom: 24.0,
        right: 32.0,
    });
    content.addArrangedSubview(&icon);
    content.addArrangedSubview(&name);
    content.addArrangedSubview(&version);
    content.addArrangedSubview(&copyright);
    content.addArrangedSubview(&license);
    content.addArrangedSubview(&website);
    content.layoutSubtreeIfNeeded();
    let size = content.fittingSize();

    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        NSPanel::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), size),
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
        NSBackingStoreType::Buffered,
        false,
    );
    // SAFETY: The panel is owned by ABOUT_PANEL and must remain valid after closing it.
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setTitle(&NSString::from_str(&format!("About {}", app_name(app))));
    panel.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    panel.setTitlebarAppearsTransparent(true);
    panel.setFloatingPanel(false);
    panel.setHidesOnDeactivate(false);
    panel.setMovableByWindowBackground(true);
    panel.setContentView(Some(&content));
    panel.setContentSize(size);
    panel.center();

    Ok(AboutPanel {
        panel,
        _link_target: link_target,
    })
}

fn label(text: &str, font: &NSFont, color: &NSColor, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    field.setFont(Some(font));
    field.setTextColor(Some(color));
    field.setAlignment(NSTextAlignment::Center);
    field.setTranslatesAutoresizingMaskIntoConstraints(false);
    field.sizeToFit();
    field
}
