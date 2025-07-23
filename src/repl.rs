use crate::icon::Icon;
use crate::icon::IconName;
use crate::input::*;
use crate::state::StateModel;
use crate::theme::*;
use gpui::*;

#[derive(Clone, Debug, IntoElement)]
pub struct NreplRequest {
    pub id: usize,
    pub req: SharedString,
}

impl NreplRequest {
    fn delete(self: &mut Self, app: &mut App) {
        StateModel::update(
            |state, app| {
                state.remove(self.id, app);
            },
            app,
        );
    }
}

impl RenderOnce for NreplRequest {
    fn render(self, _: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = app.global::<Theme>();
        div()
            .flex()
            .justify_between()
            .items_center()
            .py_2()
            .px_4()
            .border_t_1()
            .border_color(theme.crust_light)
            .hover(|s| s.bg(theme.base_blur))
            .text_xl()
            .child(self.req.clone())
            .child(
                div()
                    .flex()
                    .border_1()
                    .pl_2()
                    .pb_2()
                    .pt_2()
                    .pr_1()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Trash))
                    .on_mouse_down(MouseButton::Left, move |_, _, app| self.clone().delete(app)),
            )
    }
}

#[derive(Clone, Debug, IntoElement)]
pub struct NreplRequestResponse {
    pub id: usize,
    pub req: SharedString,
    pub resp: SharedString,
}

impl RenderOnce for NreplRequestResponse {
    fn render(self, _: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = app.global::<Theme>();
        div()
            .flex()
            .justify_between()
            .items_center()
            .py_2()
            .px_4()
            .border_t_1()
            .border_color(theme.crust_light)
            .hover(|s| s.bg(theme.base_blur))
            .text_xl()
            .children([self.req.clone(), self.resp.clone()])
    }
}

pub struct EvaluatedExprList {
    state: ListState,
}

impl Render for EvaluatedExprList {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(list(self.state.clone()).w_full().h_full())
    }
}

impl EvaluatedExprList {
    pub fn new(app: &mut App) -> Entity<Self> {
        app.new(|cx| {
            let state = cx.global::<StateModel>().inner.clone();
            cx.subscribe(&state, |this: &mut EvaluatedExprList, model, _event, cx| {
                let items = model.read(cx).items.clone();
                this.state = ListState::new(
                    items.len(),
                    ListAlignment::Bottom,
                    Pixels(20.),
                    move |idx, _win, _app| {
                        let item = items.get(idx).unwrap().clone();
                        div().child(item).into_any_element()
                    },
                );
                cx.notify();
            })
            .detach();

            EvaluatedExprList {
                state: ListState::new(0, ListAlignment::Bottom, Pixels(20.), move |_, _, _| {
                    div().into_any_element()
                }),
            }
        })
    }
}

pub struct InputControl {
    text_input: Entity<TextInput>,
}

impl InputControl {
    pub fn new(app: &mut App) -> Entity<Self> {
        app.new(|app| InputControl {
            text_input: app.new(|cx| TextInput {
                focus_handle: cx.focus_handle(),
                content: "".into(),
                placeholder: "Enter Clojure Expression...".into(),
                selected_range: 0..0,
                selection_reversed: false,
                marked_range: None,
                last_layout: None,
                last_bounds: None,
                is_selecting: false,
            }),
        })
    }
    fn submit(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        StateModel::update(
            |this, cx| {
                let item = NreplRequest {
                    id: this.inner.clone().read(cx).count,
                    req: self.text_input.read(cx).content.clone(),
                };
                this.push(item, cx);
            },
            cx,
        );

        self.text_input
            .update(cx, |text_input, _cx| text_input.reset());
        cx.notify();
    }
}

impl Render for InputControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let input = div()
            .flex()
            .flex_grow()
            .p_1()
            .rounded_md()
            .bg(theme.mantle)
            .border_1()
            .border_color(theme.crust)
            .child(self.text_input.clone());

        let button = div()
            .flex()
            .justify_center()
            .items_center()
            .p_1()
            .bg(theme.surface0)
            .min_w(px(42.0))
            .rounded_md()
            .cursor_pointer()
            .hover(|x| x.bg(theme.surface1))
            .border_color(theme.crust)
            .border_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Plus)),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::submit));

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().flex().gap_1().mt(px(10.)).child(input).child(button))
    }
}

pub struct NreplClientApp {
    pub list_view: Entity<EvaluatedExprList>,
    pub input_view: Entity<InputControl>,
}

impl NreplClientApp {
    pub fn new(app: &mut App) -> Entity<Self> {
        let list_view = EvaluatedExprList::new(app);
        let input_view = InputControl::new(app);
        app.new(|_| NreplClientApp {
            list_view,
            input_view,
        })
    }
}

impl Render for NreplClientApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let header = div()
            .flex()
            .border_b_1()
            .border_color(theme.crust_light)
            .justify_center()
            .pt_1()
            .child("Nrepl Client");

        let list = div()
            .flex()
            .flex_grow()
            .justify_center()
            .items_center()
            .child(self.list_view.clone());

        let controls = div()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(theme.crust_light)
            .child(
                div()
                    .flex()
                    .gap_1()
                    .mb_2()
                    .mx_2()
                    .child(self.input_view.clone()),
            );

        let todos_app = div()
            .flex()
            .flex_grow()
            .flex_col()
            .size_full()
            .justify_between()
            .gap_1()
            .child(list)
            .child(controls);

        div()
            .rounded_xl()
            .border_1()
            .border_color(theme.overlay0)
            .size_full()
            .child(
                div()
                    .bg(theme.base_blur)
                    .rounded_xl()
                    .flex()
                    .flex_col()
                    .size_full()
                    .justify_between()
                    .text_color(theme.text)
                    .child(header)
                    .child(todos_app),
            )
    }
}
