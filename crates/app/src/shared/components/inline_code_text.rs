//! Inline code backgrounds follow the shaped text, preserving natural wrapping.
use crate::tokens::Tokens;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, StyledText, Window, point, size,
};
use std::ops::Range;

pub struct InlineCodeText {
    pub text: StyledText,
    pub code_ranges: Vec<Range<usize>>,
}
impl IntoElement for InlineCodeText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for InlineCodeText {
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.text.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.text.prepaint(id, inspector, bounds, state, window, cx);
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut (),
        prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = Tokens::text_md_leading();
        for range in &self.code_ranges {
            let Some(start) = self.text.layout().position_for_index(range.start) else {
                continue;
            };
            let Some(end) = self.text.layout().position_for_index(range.end) else {
                continue;
            };
            let mut y = start.y;
            while y <= end.y {
                let left = if y == start.y { start.x } else { bounds.left() };
                let right = if y == end.y { end.x } else { bounds.right() };
                if right > left {
                    let rect = Bounds::new(
                        point(left, y + Tokens::spacing_0p5()),
                        size(right - left, line_height - Tokens::spacing_1()),
                    );
                    window.paint_quad(
                        gpui::fill(rect, Tokens::code_bg()).corner_radii(Tokens::radius_xs()),
                    );
                }
                y += line_height;
            }
        }
        self.text
            .paint(id, inspector, bounds, state, prepaint, window, cx);
    }
}
