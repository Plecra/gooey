use gooey::styles::components::FontFamily;
use gooey::styles::FontFamilyList;
use gooey::widget::MakeWidget;
use gooey::Run;
use kludgine::cosmic_text::FamilyOwned;
use kludgine::figures::units::Lp;

fn main() -> gooey::Result {
    include_str!("./nested-scroll.rs")
        .vertical_scroll()
        .with(&FontFamily, FontFamilyList::from(FamilyOwned::Monospace))
        .height(Lp::inches(3))
        .and(
            include_str!("./canvas.rs")
                .vertical_scroll()
                .with(&FontFamily, FontFamilyList::from(FamilyOwned::Monospace))
                .height(Lp::inches(3)),
        )
        .into_rows()
        .vertical_scroll()
        .expand()
        .run()
}
