mod assets;
mod icon;
mod input;
mod nrepl_client;
mod repl;
mod state;
mod theme;
mod window;

use assets::*;
use gpui::*;
use input::*;
use repl::*;
use state::*;
use std::env;
use theme::*;
use window::*;

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("delete", Delete, None),
            KeyBinding::new("left", Left, None),
            KeyBinding::new("right", Right, None),
            KeyBinding::new("shift-left", SelectLeft, None),
            KeyBinding::new("shift-right", SelectRight, None),
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("home", Home, None),
            KeyBinding::new("end", End, None),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, None),
            KeyBinding::new("cmd-q", Quit, None),
        ]);

        // Bring the menu bar to the foreground (so you can see the menu bar)
        cx.activate(true);
        // Register the `quit` function so it can be referenced by the `MenuItem::action` in the menu bar
        cx.on_action(quit);
        // Add menu items
        cx.set_menus(vec![Menu {
            name: "set_menus".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        }]);

        let options = get_window_options(cx);
        let args: Vec<String> = env::args().collect();
        let port: u16 = args[1]
            .clone()
            .to_string()
            .parse()
            .expect("Failed to parse port to u16");
        cx.open_window(options, move |win, app| {
            blur_window(win);
            StateModel::init(app, port);
            //Nrepl::init(app);
            Theme::init(app);
            NreplClientApp::new(app)
        })
        .unwrap();
    });
}

// Associate actions using the `actions!` macro (or `impl_actions!` macro)
actions!(set_menus, [Quit]);

// Define the quit function that is registered with the AppContext
fn quit(_: &Quit, app: &mut App) {
    println!("Gracefully quitting the application . . .");
    app.quit();
}
