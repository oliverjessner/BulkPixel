# Input format fixtures

These small, synthetic images are included so conversion tests do not need
ImageMagick, `cjxl`, network access, or a native JPEG XL library at runtime.
GIF, TIFF, and JPEG XL images are 8 × 4 pixels.

- `rgb8-codestream.jxl` and `rgb8-container.jxl`: lossless RGB8 images with
  constant pixel values `[48, 96, 144]`. Encoded with libjxl `cjxl` 0.12.0 using
  `-d 0 -e 1 --container=0` and `--container=1`, respectively.
- `rgba16.jxl`: lossless RGBA16 container with constant pixel values
  `[12345, 23456, 34567, 45678]`, encoded with `cjxl -d 0 -e 1 --container=1`.
- `multipage16.tiff`: two RGBA16 pages encoded with ImageMagick using
  `magick first.png second.png -depth 16 -compress zip multipage16.tiff`.
  The first page has the same values as `rgba16.jxl`; the second page has
  `[60000, 50000, 40000, 65535]`.
- `animated.gif`: two RGBA8 frames encoded with ImageMagick using
  `magick -delay 10 -dispose background first.png second.png -loop 0 animated.gif`.
  The first frame has `[240, 16, 32, 255]` in the left half and transparent pixels
  in the right half. The second frame is solid `[16, 32, 240, 255]`.
- `animated.jxl`: the same animation, converted with
  `cjxl animated.gif animated.jxl -d 0 -e 1 --container=1`.
- `multi-size.ico`: PNG-backed entries in directory order 256 × 256 RGBA8
  `[40, 80, 120, 128]`, 64 × 64 RGB8 `[10, 220, 30]`, and 16 × 16 RGBA8
  `[240, 10, 20, 255]`. Exercises the 256-pixel zero dimension encoding and
  selection of the largest entry at the highest color depth.
- `color-depth.ico`: 16 × 16 RGBA8 `[40, 80, 120, 128]` followed by 64 × 64
  RGB8 `[10, 220, 30]`. Verifies that color depth takes priority over size.
- `bitmap.ico`: a 16 × 16 24-bit BMP DIB with RGB `[40, 80, 120]` and an AND
  mask making the right half transparent. Verifies legacy bitmap transparency.
  The ICO fixtures were generated with Python's `struct` and `zlib`: standard
  ICO headers plus PNG chunks or a 40-byte BITMAPINFOHEADER, bottom-up BGR pixels,
  and 32-bit-aligned AND-mask rows. No third-party assets are included.

The tests verify first-frame/first-page import, extension aliases, previews,
resizing, PNG channel-depth preservation, transparency, and inspector metadata.
