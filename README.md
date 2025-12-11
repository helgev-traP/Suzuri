# Suzuri

Suzuri is a text rendering library written in Rust. It supports text rendering on both CPU and GPU (wgpu), providing font management, text layout, and rendering capabilities.

Suzuriは、Rust製のテキストレンダリングライブラリです。CPUおよびGPU (wgpu) でのテキスト描画をサポートしており、フォント管理、テキストレイアウト、レンダリング機能を提供します。

## Features / 特徴

- **Font Management**: Loads system fonts and manages custom fonts. Internally uses [fontdb](https://github.com/RazrFalcon/fontdb).
- **Text Layout**: Calculates text placement including wrapping, alignment, line spacing, etc.
- **Consistent Layout**: Ensures layout consistency and reproducibility within a two-pass layout data flow. Unlike existing libraries such as `cosmic-text` or `glyphon`, it prevents unstable behavior where reducing layout width alters line breaks even when sufficient whitespace remains.
- **Rendering**:
  - **CPU Rendering**: Drawing to image buffers.
  - **GPU Rendering**: High-speed drawing using [wgpu](https://wgpu.rs/).

---

- **フォント管理**: システムフォントの読み込みや、カスタムフォントの管理を行います。内部で [fontdb](https://github.com/RazrFalcon/fontdb) を使用しています。
- **テキストレイアウト**: 折り返し、整列、行間設定などを含むテキスト配置計算を行います。
- **レイアウトの整合性**: `cosmic-text` や `glyphon` などの既存ライブラリとは異なり、2パスレイアウトデータフロー内での整合性と再現性を確保しています。十分な余白がある状態でレイアウト幅を縮小しても、改行位置などの結果が不意に変わることのない安定した挙動を提供します。
- **レンダリング**:
  - **CPUレンダリング**: 画像バッファへの描画。
  - **GPUレンダリング**: [wgpu](https://wgpu.rs/) を使用した高速な描画。

## Installation / インストール

Add the following to your `Cargo.toml`.

`Cargo.toml` に以下を追加してください。

```toml
[dependencies]
suzuri = "0.1.0"
```

To use wgpu features, enable the `wgpu` feature.

wgpu機能を使用する場合は、`wgpu` featureを有効にしてください。

```toml
[dependencies]
suzuri = { version = "0.1.0", features = ["wgpu"] }
```

## Usage / 使い方

### 1. Prepare Fonts / フォントの準備

Use `FontStorage` to load fonts. You can load system fonts or query specific fonts.

`FontStorage` を使用してフォントを読み込みます。システムフォントをロードしたり、特定のフォントをクエリすることができます。

```rust
use suzuri::font_storage::FontStorage;
use suzuri::fontdb::{self, Family, Query};

let mut font_storage = FontStorage::new();
font_storage.load_system_fonts();

// Query a font / フォントの検索
let font_id = font_storage
    .query(&Query {
        families: &[Family::Name("Arial"), Family::SansSerif],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    })
    .map(|(id, _)| id)
    .expect("Font not found");
```

For details on font queries, please refer to the [fontdb documentation](https://docs.rs/fontdb/latest/fontdb/struct.Query.html).

フォントクエリの詳細については、[fontdbのドキュメント](https://docs.rs/fontdb/latest/fontdb/struct.Query.html)を参照してください。

### 2. Create Text Data / テキストデータの作成

Define the content and style of the text you want to render.

描画したいテキストの内容とスタイルを定義します。

```rust
use suzuri::text::{TextData, TextElement};

// Color type is user-definable / 色の型はユーザー定義可能です
#[derive(Clone, Copy, Debug)]
struct MyColor { r: f32, g: f32, b: f32, a: f32 }

let mut data = TextData::new();
data.append(TextElement {
    text: "Hello, Suzuri!".to_string(),
    font_id,
    size: 32.0,
    color: MyColor { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
});
```

### 3. Calculate Layout / レイアウトの計算

Configure layout settings with `TextLayoutConfig` and calculate the placement.

`TextLayoutConfig` でレイアウト設定を行い、配置を計算します。

```rust
use suzuri::text::{TextLayoutConfig, HorizontalAlign, VerticalAlign, WrapStyle};

let config = TextLayoutConfig {
    max_width: Some(800.0),
    max_height: None,
    horizontal_align: HorizontalAlign::Left,
    vertical_align: VerticalAlign::Top,
    line_height_scale: 1.2,
    wrap_style: WrapStyle::WordWrap,
    ..Default::default()
};

let layout = data.layout(&config, &mut font_storage);
```

### 4. Rendering / レンダリング

#### GPU Rendering (wgpu) / GPUレンダリング (wgpu)

Use `WgpuRenderer` to draw. You need to set up `wgpu::Device` and `wgpu::Queue` beforehand.

`WgpuRenderer` を使用して描画します。事前に `wgpu::Device` や `wgpu::Queue` のセットアップが必要です。

```rust
use suzuri::renderer::wgpu_renderer::WgpuRenderer;
use suzuri::renderer::gpu_renderer::GpuCacheConfig;
use std::num::NonZeroUsize;

// Cache configuration / キャッシュ設定
let cache_configs = vec![
    GpuCacheConfig {
        tile_size: NonZeroUsize::new(32).unwrap(),
        tiles_per_axis: NonZeroUsize::new(16).unwrap(),
        texture_size: NonZeroUsize::new(512).unwrap(),
    },
];

let mut renderer = WgpuRenderer::new(&device, &cache_configs, &[texture_format]);

// Draw inside a render pass / レンダーパス内での描画
renderer.render(&layout, &device, &queue, &mut rpass);
```

#### CPU Rendering / CPUレンダリング

Use `CpuRenderer` to get pixel data.

`CpuRenderer` を使用してピクセルデータを取得します。

```rust
use suzuri::renderer::cpu_renderer::CpuRenderer;

let mut renderer = CpuRenderer::new();
let pixel_buffer = renderer.render(&layout, &mut font_storage);
```

## License / ライセンス

MIT OR Apache-2.0
