//! Names the element an input reached, for the interaction recorder.
//!
//! `test_support()` here replaces `gpui_kit::TestSupportExt`: it keeps the
//! test-only observation and also wraps the element in [`Targeted`], which
//! exists in every build. While the recorder runs, a targeted element keeps its
//! hitbox and focus handle in GPUI element state, so a click resolves to the
//! deepest hovered element and a key to the deepest focused one. Element state
//! also survives cached-view replay, so cached subtrees still resolve.
//!
//! A target is named by its element id: a `Name` id up to its first `:`, or a
//! `NamedInteger` id's name. Many ids are built from container names, keys and
//! other cluster data, sometimes with no `:` before them, and that data has the
//! same lowercase kebab shape as the static ids. So a name is recorded only when
//! `build.rs` found it as the leading text of a string literal in `src/`
//! (`TARGET_NAMES`); any other id, including ids of other kinds, is recorded as
//! `dynamic`. Labels and text are never read.
use gpui_kit::{
    App, Bounds, Element, ElementId, FocusHandle, GlobalElementId, Hitbox, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, Pixels, Window,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static REGISTRY: RefCell<HashMap<GlobalElementId, Weak<Target>>> = RefCell::default();
}

struct Target {
    name: String,
    depth: usize,
    hitbox: RefCell<Option<Hitbox>>,
    focus: RefCell<Option<FocusHandle>>,
}

/// Starts or stops registering targets. Starting needs a full redraw so
/// already painted and cached elements register.
pub(crate) fn set_recording(recording: bool) {
    RECORDING.set(recording);
    if !recording {
        REGISTRY.with_borrow_mut(HashMap::clear);
    }
}

fn deepest(matches: impl Fn(&Target) -> bool) -> Option<String> {
    REGISTRY.with_borrow_mut(|registry| {
        registry.retain(|_, target| target.strong_count() > 0);
        registry
            .values()
            .filter_map(Weak::upgrade)
            .filter(|target| matches(target))
            .max_by_key(|target| target.depth)
            .map(|target| target.name.clone())
    })
}

/// The deepest registered element under the pointer.
pub(crate) fn hovered(window: &Window) -> Option<String> {
    deepest(|target| {
        target
            .hitbox
            .borrow()
            .as_ref()
            .is_some_and(|hitbox| hitbox.is_hovered(window))
    })
}

/// The deepest registered element that holds or contains focus.
pub(crate) fn focused(window: &Window, cx: &App) -> Option<String> {
    deepest(|target| {
        target
            .focus
            .borrow()
            .as_ref()
            .is_some_and(|focus| focus.contains_focused(window, cx))
    })
}

/// Sorted static names from the program's string literals, written by `build.rs`.
const TARGET_NAMES: &[&str] = include!(concat!(env!("OUT_DIR"), "/target_names.rs"));

fn target_name(role: Option<gpui_kit::Role>, id: &ElementId) -> String {
    let name = match id {
        ElementId::Name(text) => text.split(':').next(),
        ElementId::NamedInteger(name, _) => Some(&**name),
        _ => None,
    };
    let id = name
        .filter(|name| TARGET_NAMES.binary_search(name).is_ok())
        .unwrap_or("dynamic");
    match role {
        Some(role) => format!("{} [id={id}]", format!("{role:?}").to_lowercase()),
        None => format!("[id={id}]"),
    }
}

/// What the app's `test_support()` returns.
pub(crate) type ObservedElement<E> = gpui_kit::base::ObservedElement<Targeted<E>>;

/// The app's `test_support()`. See the module comment.
pub(crate) trait TestSupportExt:
    Element<PrepaintState = Option<Hitbox>> + InteractiveElement + Sized
{
    fn test_support(self) -> ObservedElement<Self> {
        gpui_kit::TestSupportExt::test_support(Targeted {
            inner: self,
            focus: None,
        })
    }
}
impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement> TestSupportExt for E {}

/// Transparent forwarding element. See the module comment.
pub struct Targeted<E> {
    inner: E,
    focus: Option<FocusHandle>,
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement> IntoElement for Targeted<E> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement> Element for Targeted<E> {
    type RequestLayoutState = E::RequestLayoutState;
    type PrepaintState = Option<Hitbox>;
    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }
    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        let hitbox = self
            .inner
            .prepaint(id, inspector, bounds, layout, window, cx);
        if RECORDING.get()
            && let Some(global_id) = id
        {
            self.register(global_id, hitbox.clone(), window);
        }
        hitbox
    }
    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        prepaint: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector, bounds, layout, prepaint, window, cx);
    }
    fn a11y_role(&self) -> Option<gpui_kit::Role> {
        self.inner.a11y_role()
    }
    fn write_a11y_info(&self, node: &mut gpui_kit::accesskit::Node) {
        self.inner.write_a11y_info(node);
    }
    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Option<Hitbox>,
        builder: &mut gpui_kit::A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(prepaint, builder);
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement> Targeted<E> {
    fn register(&self, global_id: &GlobalElementId, hitbox: Option<Hitbox>, window: &mut Window) {
        let role = self.inner.a11y_role();
        let focus = self.focus.clone();
        window.with_element_state(global_id, |state: Option<Rc<Target>>, _| {
            let target = state.unwrap_or_else(|| {
                Rc::new(Target {
                    name: target_name(role, global_id.last().expect("element ids are nonempty")),
                    depth: global_id.len(),
                    hitbox: RefCell::default(),
                    focus: RefCell::default(),
                })
            });
            *target.hitbox.borrow_mut() = hitbox;
            *target.focus.borrow_mut() = focus;
            REGISTRY.with_borrow_mut(|registry| {
                registry.insert(global_id.clone(), Rc::downgrade(&target));
            });
            ((), target)
        });
    }
}

impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement + gpui_kit::Styled>
    gpui_kit::Styled for Targeted<E>
{
    fn style(&mut self) -> &mut gpui_kit::StyleRefinement {
        self.inner.style()
    }
}
impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement> InteractiveElement
    for Targeted<E>
{
    fn interactivity(&mut self) -> &mut gpui_kit::Interactivity {
        self.inner.interactivity()
    }
    fn track_focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self.inner = self.inner.track_focus(focus);
        self
    }
}
impl<E: Element<PrepaintState = Option<Hitbox>> + InteractiveElement + gpui_kit::ParentElement>
    gpui_kit::ParentElement for Targeted<E>
{
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui_kit::AnyElement>) {
        self.inner.extend(elements);
    }
}
impl<E: Element<PrepaintState = Option<Hitbox>> + gpui_kit::StatefulInteractiveElement>
    gpui_kit::StatefulInteractiveElement for Targeted<E>
{
}

#[cfg(test)]
mod tests {
    use super::{TARGET_NAMES, target_name};
    use gpui_kit::{ElementId, Role};

    // Literals in this module are in `src/` too, so the names the tests expect
    // in the table are assembled at run time from parts. They are then in the
    // table only because the app's own ids use them.

    #[test]
    fn cluster_data_before_a_colon_is_dynamic() {
        // Longer than every table entry, so it cannot be one.
        let longest = TARGET_NAMES.iter().map(|name| name.len()).max();
        let pod = format!("payments-api-{}", "x".repeat(longest.unwrap_or(0)));
        let id = ElementId::Name(format!("{pod}:logs").into());
        assert_eq!(target_name(None, &id), "[id=dynamic]");
    }

    #[test]
    fn field_controls_id_without_a_colon_is_dynamic() {
        // The shape `inspector/actions/fields.rs` builds for a container and key.
        let id = ElementId::Name("Environment value web API_KEY controls".into());
        assert_eq!(target_name(Some(Role::Button), &id), "button [id=dynamic]");
    }

    #[test]
    fn literal_prefix_keeps_only_the_static_part() {
        // `inspector/interaction_report.rs` builds `setting-row:{title}`.
        let id = ElementId::Name(format!("{}-row:payments-api", "setting").into());
        assert_eq!(target_name(None, &id), "[id=setting-row]");
    }

    #[test]
    fn named_integer_records_its_literal_name() {
        // `inspector/interaction_report.rs` builds `("interaction-row", index)`.
        let id = ElementId::NamedInteger(format!("{}-row", "interaction").into(), 3);
        assert_eq!(target_name(None, &id), "[id=interaction-row]");
    }
}
