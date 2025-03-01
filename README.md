# Gooey

![Gooey is considered alpha and unsupported](https://img.shields.io/badge/status-alpha-orange)
[![crate version](https://img.shields.io/crates/v/gooey.svg)](https://crates.io/crates/gooey)
[![Documentation for `main` branch](https://img.shields.io/badge/docs-main-informational)](https://gooey.rs/main/gooey/)

Gooey is an experimental Graphical User Interface (GUI) crate for the Rust
programming language. It is powered by:

- [`Kludgine`][kludgine], a 2d graphics library powered by:
  - [`winit`][winit] for windowing/input
  - [`wgpu`][wgpu] for graphics
  - [`cosmic_text`][cosmic_text] for text layout + rasterization
- [`palette`][palette] for OKLab-based HSL color calculations
- [`arboard`][arboard] for clipboard support
- [`figures`][figures] for integer-based 2d math

## Getting Started with Gooey

The [`Widget`][widget] trait is the building block of Gooey: Every user
interface element implements `Widget`. The `Widget` trait
[documentation][widget] has an overview of how Gooey works. A list of built-in
widgets can be found in the [`gooey::widgets`][widgets] module.

Gooey uses a reactive data model. To see [an example][button-example] of how
reactive data models work, consider this example that displays a button that
increments its own label:

```rust,ignore
fn main() -> gooey::Result {
    // Create a dynamic usize.
    let count = Dynamic::new(0_isize);
    // Create a dynamic that contains `count.to_string()`
    let count_label = count.map_each(ToString::to_string);

    // Create a new button whose text is our dynamic string.
    count_label
        .into_button()
        // Set the `on_click` callback to a closure that increments the counter.
        .on_click(move |_| count.set(count.get() + 1))
        // Run the application
        .run()
}
```

A great way to learn more about Gooey is to explore the [examples
directory][examples]. Nearly every feature in Gooey was initially tested by
creating an example.

## Project Status

This project is early in development, but is quickly becoming a decent
framework. It is considered alpha and unsupported at this time, and the primary
focus for [@ecton][ecton] is to use this for his own projects. Feature requests
and bug fixes will be prioritized based on @ecton's own needs.

If you would like to contribute, bug fixes are always appreciated. Before
working on a new feature, please [open an issue][issues] proposing the feature
and problem it aims to solve. Doing so will help prevent friction in merging
pull requests, as it ensures changes fit the vision the maintainers have for
Gooey.

[widget]: https://gooey.rs/main/gooey/widget/trait.Widget.html
[widgets]: https://gooey.rs/main/gooey/widgets/index.html
[button-example]: https://github.com/khonsulabs/gooey/tree/main/examples/basic-button.rs
[examples]: https://github.com/khonsulabs/gooey/tree/main/examples/
[kludgine]: https://github.com/khonsulabs/kludgine
[figures]: https://github.com/khonsulabs/figures
[wgpu]: https://github.com/gfx-rs/wgpu
[winit]: https://github.com/rust-windowing/winit
[cosmic_text]: https://github.com/pop-os/cosmic-text
[palette]: https://github.com/Ogeon/palette
[arboard]: https://github.com/1Password/arboard
[ecton]: https://github.com/khonsulabs/ecton
[issues]: https://github.com/khonsulabs/gooey/issues

## Open-source Licenses

This project, like all projects from [Khonsu Labs](https://khonsulabs.com/), is open-source.
This repository is available under the [MIT License](./LICENSE-MIT) or the
[Apache License 2.0](./LICENSE-APACHE).

To learn more about contributing, please see [CONTRIBUTING.md](./CONTRIBUTING.md).
