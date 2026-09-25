use gtk::gdk::prelude::GdkCairoContextExt;
use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use gtk::{
    Application, ApplicationWindow, Box as GtkBox, DrawingArea, GestureClick, GestureDrag, Label,
    Orientation,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use lychnos_core::{
    diagnostics::DiagnosticLevel,
    presentation::{CompanionPresentationEnvelope, CompanionPresentationState},
    runtime::RuntimeMode,
};

const APP_ID: &str = "org.lychnos.prototype.shell";
const DEFAULT_TOP_MARGIN: i32 = 36;
const DEFAULT_RIGHT_MARGIN: i32 = 42;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShellPreferences {
    status_visible: bool,
    ghosted: bool,
    position_locked: bool,
}

impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            status_visible: true,
            ghosted: false,
            position_locked: false,
        }
    }
}

// Excited and Speaking are part of the canonical expression vocabulary but
// do not have live core states until later interaction/voice phases.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompanionExpression {
    Neutral,
    Happy,
    Thinking,
    Excited,
    Focused,
    Listening,
    Speaking,
}

fn main() {
    let application = Application::builder().application_id(APP_ID).build();
    application.connect_activate(build_ui);
    application.run();
}

fn build_ui(app: &Application) {
    install_css();

    let initial_state = load_presentation_snapshot().unwrap_or_else(default_presentation_state);
    let state = Rc::new(RefCell::new(initial_state));
    let preferences = Rc::new(RefCell::new(load_shell_preferences()));
    let dragging = Rc::new(Cell::new(false));
    let body = build_body(Rc::clone(&state), Rc::clone(&dragging));
    let status = build_status_card(&state.borrow());

    let layout = GtkBox::new(Orientation::Vertical, 8);
    layout.set_halign(gtk::Align::Center);
    layout.append(&body);
    layout.append(&status.card);

    let initial_preferences = *preferences.borrow();
    status.card.set_visible(initial_preferences.status_visible);
    let initial_opacity = if initial_preferences.ghosted {
        0.28
    } else {
        1.0
    };
    body.set_opacity(initial_opacity);
    status.card.set_opacity(initial_opacity);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Lychnos Shell Prototype")
        .child(&layout)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();
    window.set_namespace(Some("lychnos-prototype"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_exclusive_zone(0);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, true);

    let (top_margin, right_margin) = load_shell_position();
    window.set_margin(Edge::Top, top_margin);
    window.set_margin(Edge::Right, right_margin);

    install_shell_interactions(
        &window,
        &body,
        &status.card,
        top_margin,
        right_margin,
        Rc::clone(&dragging),
        Rc::clone(&preferences),
    );
    install_live_presentation_updates(Rc::clone(&state), &body, &status);

    let restore_action = gtk::gio::SimpleAction::new("restore", None);
    {
        let window = window.clone();
        let body = body.clone();
        let status = status.card.clone();
        let preferences = Rc::clone(&preferences);
        restore_action.connect_activate(move |_, _| {
            set_click_through(&window, false);
            body.set_opacity(1.0);
            status.set_opacity(1.0);
            preferences.borrow_mut().ghosted = false;
            save_shell_preferences(*preferences.borrow());
            window.set_visible(true);
            window.present();
            save_shell_presence("visible");
        });
    }
    app.add_action(&restore_action);

    window.connect_close_request(|_| {
        save_shell_presence("closed");
        gtk::glib::Propagation::Proceed
    });

    if load_shell_presence().as_deref() == Some("hidden") {
        window.set_visible(false);
        save_shell_presence("hidden");
    } else {
        window.present();
        if initial_preferences.ghosted {
            set_click_through(&window, true);
            save_shell_presence("ghosted");
        } else {
            save_shell_presence("visible");
        }
    }
}

fn install_shell_interactions(
    window: &ApplicationWindow,
    body: &DrawingArea,
    status: &GtkBox,
    top_margin: i32,
    right_margin: i32,
    dragging: Rc<Cell<bool>>,
    preferences: Rc<RefCell<ShellPreferences>>,
) {
    let current_top = Rc::new(Cell::new(top_margin));
    let current_right = Rc::new(Cell::new(right_margin));
    let drag_start_top = Rc::new(Cell::new(top_margin));
    let drag_start_right = Rc::new(Cell::new(right_margin));
    let initial_preferences = *preferences.borrow();
    status.set_visible(initial_preferences.status_visible);

    let drag = GestureDrag::new();

    {
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let drag_start_top = Rc::clone(&drag_start_top);
        let drag_start_right = Rc::clone(&drag_start_right);
        let dragging = Rc::clone(&dragging);
        let preferences = Rc::clone(&preferences);
        let body = body.clone();

        drag.connect_drag_begin(move |_, _, _| {
            if preferences.borrow().position_locked {
                dragging.set(false);
                return;
            }

            drag_start_top.set(current_top.get());
            drag_start_right.set(current_right.get());
            dragging.set(true);
            body.queue_draw();
        });
    }

    {
        let window = window.clone();
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let drag_start_top = Rc::clone(&drag_start_top);
        let drag_start_right = Rc::clone(&drag_start_right);
        let preferences = Rc::clone(&preferences);

        drag.connect_drag_update(move |_, offset_x, offset_y| {
            if preferences.borrow().position_locked {
                return;
            }

            let requested_top = drag_start_top.get() + offset_y.round() as i32;
            let requested_right = drag_start_right.get() - offset_x.round() as i32;
            let (top, right) = clamp_shell_position(&window, requested_top, requested_right);

            current_top.set(top);
            current_right.set(right);
            window.set_margin(Edge::Top, top);
            window.set_margin(Edge::Right, right);
        });
    }

    {
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let dragging = Rc::clone(&dragging);
        let preferences = Rc::clone(&preferences);
        let body = body.clone();

        drag.connect_drag_end(move |_, _, _| {
            dragging.set(false);
            body.queue_draw();

            if !preferences.borrow().position_locked {
                save_shell_position(current_top.get(), current_right.get());
            }
        });
    }

    body.add_controller(drag);

    let click = GestureClick::new();
    click.set_button(1);
    {
        let status = status.clone();
        let preferences = Rc::clone(&preferences);
        click.connect_released(move |_, press_count, _, _| {
            if press_count == 2 {
                let visible = !preferences.borrow().status_visible;
                status.set_visible(visible);
                preferences.borrow_mut().status_visible = visible;
                save_shell_preferences(*preferences.borrow());
            }
        });
    }
    body.add_controller(click);

    install_context_menu(
        window,
        body,
        status,
        Rc::clone(&current_top),
        Rc::clone(&current_right),
        Rc::clone(&preferences),
    );
}

fn install_context_menu(
    window: &ApplicationWindow,
    body: &DrawingArea,
    status: &GtkBox,
    current_top: Rc<Cell<i32>>,
    current_right: Rc<Cell<i32>>,
    preferences: Rc<RefCell<ShellPreferences>>,
) {
    let popover = gtk::Popover::new();
    popover.set_has_arrow(true);
    popover.set_parent(body);
    popover.add_css_class("lychnos-menu");

    let menu = GtkBox::new(Orientation::Vertical, 3);
    menu.add_css_class("lychnos-menu-box");

    let initial_preferences = *preferences.borrow();
    let ghost_button = gtk::Button::with_label(if initial_preferences.ghosted {
        "Exit Ghost Mode"
    } else {
        "Ghost Mode"
    });
    ghost_button.add_css_class("lychnos-menu-item");
    let status_button = gtk::Button::with_label(if initial_preferences.status_visible {
        "Hide status"
    } else {
        "Show status"
    });
    status_button.add_css_class("lychnos-menu-item");
    let lock_button = gtk::Button::with_label(if initial_preferences.position_locked {
        "Unlock position"
    } else {
        "Lock position"
    });
    lock_button.add_css_class("lychnos-menu-item");
    let minimize_button = gtk::Button::with_label("Minimize to top bar");
    minimize_button.add_css_class("lychnos-menu-item");
    let reset_button = gtk::Button::with_label("Reset position");
    reset_button.add_css_class("lychnos-menu-item");
    let close_button = gtk::Button::with_label("Close Lychnos");
    close_button.add_css_class("lychnos-menu-item");
    close_button.add_css_class("destructive-action");

    menu.append(&ghost_button);
    menu.append(&status_button);
    menu.append(&lock_button);
    menu.append(&gtk::Separator::new(Orientation::Horizontal));
    menu.append(&minimize_button);
    menu.append(&reset_button);
    menu.append(&gtk::Separator::new(Orientation::Horizontal));
    menu.append(&close_button);
    popover.set_child(Some(&menu));

    {
        let window = window.clone();
        let body = body.clone();
        let status = status.clone();
        let popover = popover.clone();
        let preferences = Rc::clone(&preferences);

        ghost_button.connect_clicked(move |button| {
            let next = !preferences.borrow().ghosted;
            let opacity = if next { 0.28 } else { 1.0 };
            body.set_opacity(opacity);
            status.set_opacity(opacity);
            preferences.borrow_mut().ghosted = next;
            save_shell_preferences(*preferences.borrow());
            set_click_through(&window, next);
            save_shell_presence(if next { "ghosted" } else { "visible" });
            button.set_label(if next {
                "Exit Ghost Mode"
            } else {
                "Ghost Mode"
            });
            popover.popdown();
        });
    }

    {
        let status = status.clone();
        let popover = popover.clone();
        let preferences = Rc::clone(&preferences);

        status_button.connect_clicked(move |button| {
            let visible = !preferences.borrow().status_visible;
            status.set_visible(visible);
            preferences.borrow_mut().status_visible = visible;
            save_shell_preferences(*preferences.borrow());
            button.set_label(if visible {
                "Hide status"
            } else {
                "Show status"
            });
            popover.popdown();
        });
    }

    {
        let popover = popover.clone();
        let preferences = Rc::clone(&preferences);

        lock_button.connect_clicked(move |button| {
            let locked = !preferences.borrow().position_locked;
            preferences.borrow_mut().position_locked = locked;
            save_shell_preferences(*preferences.borrow());
            button.set_label(if locked {
                "Unlock position"
            } else {
                "Lock position"
            });
            popover.popdown();
        });
    }

    {
        let window = window.clone();
        let popover = popover.clone();

        minimize_button.connect_clicked(move |_| {
            popover.popdown();
            save_shell_presence("hidden");
            window.set_visible(false);
        });
    }

    {
        let window = window.clone();
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let popover = popover.clone();

        reset_button.connect_clicked(move |_| {
            current_top.set(DEFAULT_TOP_MARGIN);
            current_right.set(DEFAULT_RIGHT_MARGIN);
            window.set_margin(Edge::Top, DEFAULT_TOP_MARGIN);
            window.set_margin(Edge::Right, DEFAULT_RIGHT_MARGIN);
            save_shell_position(DEFAULT_TOP_MARGIN, DEFAULT_RIGHT_MARGIN);
            popover.popdown();
        });
    }

    {
        let window = window.clone();

        close_button.connect_clicked(move |_| {
            save_shell_presence("closed");
            window.close();
        });
    }

    let context_click = GestureClick::new();
    context_click.set_button(3);
    {
        let popover = popover.clone();
        let status_button = status_button.clone();
        let lock_button = lock_button.clone();
        let ghost_button = ghost_button.clone();
        let preferences = Rc::clone(&preferences);

        context_click.connect_pressed(move |_, _, x, y| {
            let current = *preferences.borrow();
            status_button.set_label(if current.status_visible {
                "Hide status"
            } else {
                "Show status"
            });
            lock_button.set_label(if current.position_locked {
                "Unlock position"
            } else {
                "Lock position"
            });
            ghost_button.set_label(if current.ghosted {
                "Exit Ghost Mode"
            } else {
                "Ghost Mode"
            });
            let rect = gtk::gdk::Rectangle::new(x.round() as i32, y.round() as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
        });
    }
    body.add_controller(context_click);
}

fn clamp_shell_position(window: &ApplicationWindow, top: i32, right: i32) -> (i32, i32) {
    let Some(surface) = window.surface() else {
        return (top.max(0), right.max(0));
    };

    let display = gtk::prelude::WidgetExt::display(window);
    let Some(monitor) = display.monitor_at_surface(&surface) else {
        return (top.max(0), right.max(0));
    };

    let geometry = monitor.geometry();
    let window_width = window.width().max(1);
    let window_height = window.height().max(1);
    let max_right = (geometry.width() - window_width).max(0);
    let max_top = (geometry.height() - window_height).max(0);

    (top.clamp(0, max_top), right.clamp(0, max_right))
}

fn set_click_through(window: &ApplicationWindow, enabled: bool) {
    let Some(surface) = window.surface() else {
        return;
    };

    if enabled {
        let empty_region = gtk::cairo::Region::create();
        surface.set_input_region(Some(&empty_region));
    } else {
        surface.set_input_region(None);
    }
}

fn shell_preferences_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path).join("lychnos/shell-preferences.conf"));
    }

    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/lychnos/shell-preferences.conf"))
}

fn load_shell_preferences() -> ShellPreferences {
    let Some(path) = shell_preferences_path() else {
        return ShellPreferences::default();
    };

    let Ok(contents) = fs::read_to_string(path) else {
        return ShellPreferences::default();
    };

    let mut preferences = ShellPreferences::default();
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("status_visible=") {
            preferences.status_visible = value == "true";
        } else if let Some(value) = line.strip_prefix("ghosted=") {
            preferences.ghosted = value == "true";
        } else if let Some(value) = line.strip_prefix("position_locked=") {
            preferences.position_locked = value == "true";
        }
    }

    preferences
}

fn save_shell_preferences(preferences: ShellPreferences) {
    let Some(path) = shell_preferences_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let contents = format!(
        "status_visible={}\nghosted={}\nposition_locked={}\n",
        preferences.status_visible, preferences.ghosted, preferences.position_locked
    );
    let _ = fs::write(path, contents);
}

fn load_shell_presence() -> Option<String> {
    let path = shell_presence_path()?;
    fs::read_to_string(path)
        .ok()
        .map(|contents| contents.trim().to_string())
}

fn shell_presence_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("XDG_STATE_HOME") {
        return Some(PathBuf::from(path).join("lychnos/shell-presence"));
    }

    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/state/lychnos/shell-presence"))
}

fn save_shell_presence(state: &str) {
    let Some(path) = shell_presence_path() else {
        return;
    };

    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let _ = fs::write(
        path,
        format!(
            "{state}
"
        ),
    );
}

fn shell_position_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path).join("lychnos/shell-position.conf"));
    }

    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/lychnos/shell-position.conf"))
}

fn load_shell_position() -> (i32, i32) {
    let Some(path) = shell_position_path() else {
        return (DEFAULT_TOP_MARGIN, DEFAULT_RIGHT_MARGIN);
    };

    let Ok(contents) = fs::read_to_string(path) else {
        return (DEFAULT_TOP_MARGIN, DEFAULT_RIGHT_MARGIN);
    };

    let mut top = None;
    let mut right = None;

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("top=") {
            top = value.parse::<i32>().ok().filter(|value| *value >= 0);
        } else if let Some(value) = line.strip_prefix("right=") {
            right = value.parse::<i32>().ok().filter(|value| *value >= 0);
        }
    }

    (
        top.unwrap_or(DEFAULT_TOP_MARGIN),
        right.unwrap_or(DEFAULT_RIGHT_MARGIN),
    )
}

fn save_shell_position(top: i32, right: i32) {
    let Some(path) = shell_position_path() else {
        return;
    };

    let Some(parent) = path.parent() else {
        return;
    };

    if fs::create_dir_all(parent).is_err() {
        return;
    }

    let _ = fs::write(path, format!("top={top}\nright={right}\n"));
}

fn canonical_body_asset_path() -> PathBuf {
    if let Ok(data_dir) = std::env::var("LYCHNOS_DATA_DIR") {
        let path = PathBuf::from(data_dir).join("assets/lychnos-body-v1.png");
        if path.exists() {
            return path;
        }
    }

    let installed = if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        Some(PathBuf::from(data_home).join("lychnos/assets/lychnos-body-v1.png"))
    } else {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".local/share/lychnos/assets/lychnos-body-v1.png"))
    };

    if let Some(path) = installed.filter(|path| path.exists()) {
        return path;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/canon/lychnos-body-v1.png")
}

fn build_body(
    state: Rc<RefCell<CompanionPresentationState>>,
    dragging: Rc<Cell<bool>>,
) -> DrawingArea {
    let area = DrawingArea::new();
    area.set_content_width(210);
    area.set_content_height(210);
    area.set_tooltip_text(Some(
        "Drag to move · double-click status · right-click options",
    ));

    let asset_path = canonical_body_asset_path();
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file(&asset_path).unwrap_or_else(|error| {
        panic!(
            "canonical Lychnos PNG should load from {}: {error}",
            asset_path.display()
        )
    });
    let pixbuf = pixbuf
        .scale_simple(164, 164, gtk::gdk_pixbuf::InterpType::Bilinear)
        .expect("canonical Lychnos PNG should scale");

    let phase = Rc::new(Cell::new(0.0_f64));
    let draw_phase = Rc::clone(&phase);
    let draw_dragging = Rc::clone(&dragging);
    let draw_state = Rc::clone(&state);

    area.set_draw_func(move |_, cr, width, height| {
        let state = draw_state.borrow();
        let mode = state.runtime_mode;
        let approvals = state.pending_approval_count();
        let alert = state.latest_diagnostic.as_ref().is_some_and(|diagnostic| {
            matches!(
                diagnostic.level,
                DiagnosticLevel::Warning | DiagnosticLevel::Error
            )
        });
        let expression = expression_from_state(&state);

        let w = f64::from(width);
        let h = f64::from(height);
        let cx = w / 2.0;
        let bob = if draw_dragging.get() {
            0.0
        } else {
            draw_phase.get().sin() * 4.0
        };
        let body_x = cx - 82.0;
        let body_y = h / 2.0 - 82.0 - 8.0 + bob;
        let cy = body_y + 82.0;
        let pulse = (draw_phase.get() * 0.72).sin() * 0.06;
        let (r, g, b) = mode_accent(mode, alert);

        cr.set_source_pixbuf(&pixbuf, body_x, body_y);
        let _ = cr.paint();

        draw_expression(cr, cx, cy, expression, draw_phase.get());

        if alert || !matches!(mode, RuntimeMode::Normal) {
            cr.set_source_rgba(r, g, b, 0.20 + pulse);
            cr.set_line_width(3.0);
            cr.arc(cx, cy - 2.0, 75.0, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
        }

        if approvals > 0 {
            cr.set_source_rgba(1.0, 0.56, 0.16, 0.98);
            cr.arc(cx + 54.0, cy - 53.0, 8.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();

            cr.set_source_rgba(1.0, 0.78, 0.38, 0.36);
            cr.set_line_width(4.0);
            cr.arc(cx + 54.0, cy - 53.0, 11.0, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
        }
    });

    let animation_area = area.clone();
    let animation_dragging = Rc::clone(&dragging);
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
        if !animation_dragging.get() {
            let next = phase.get() + 0.09;
            phase.set(if next > std::f64::consts::TAU {
                next - std::f64::consts::TAU
            } else {
                next
            });
            animation_area.queue_draw();
        }
        gtk::glib::ControlFlow::Continue
    });

    area
}

fn draw_expression(
    cr: &gtk::cairo::Context,
    cx: f64,
    cy: f64,
    expression: CompanionExpression,
    phase: f64,
) {
    // The current canonical PNG has a baked-in happy face. Mask only the central
    // glass face region, then render state-driven expressions on top.
    cr.set_source_rgba(0.005, 0.008, 0.014, 0.97);
    cr.arc(cx, cy - 5.0, 43.0, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();

    let glow = 0.88 + (phase * 0.9).sin() * 0.06;
    cr.set_source_rgba(0.19, 0.90, 1.0, glow);
    cr.set_line_width(4.2);
    cr.set_line_cap(gtk::cairo::LineCap::Round);

    match expression {
        CompanionExpression::Neutral => {
            draw_eye_circle(cr, cx - 17.0, cy - 9.0, 6.0);
            draw_eye_circle(cr, cx + 17.0, cy - 9.0, 6.0);
        }
        CompanionExpression::Happy => {
            draw_happy_eye(cr, cx - 17.0, cy - 7.0);
            draw_happy_eye(cr, cx + 17.0, cy - 7.0);
            cr.move_to(cx - 11.0, cy + 15.0);
            cr.curve_to(
                cx - 4.0,
                cy + 20.0,
                cx + 4.0,
                cy + 20.0,
                cx + 11.0,
                cy + 15.0,
            );
            let _ = cr.stroke();
        }
        CompanionExpression::Thinking => {
            cr.move_to(cx - 24.0, cy - 8.0);
            cr.line_to(cx - 10.0, cy - 8.0);
            cr.move_to(cx + 10.0, cy - 8.0);
            cr.line_to(cx + 24.0, cy - 8.0);
            let _ = cr.stroke();
            cr.arc(cx + 24.0, cy + 8.0, 2.8, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
        CompanionExpression::Excited => {
            draw_happy_eye(cr, cx - 18.0, cy - 8.0);
            draw_happy_eye(cr, cx + 18.0, cy - 8.0);
            cr.move_to(cx - 12.0, cy + 14.0);
            cr.curve_to(
                cx - 4.0,
                cy + 22.0,
                cx + 4.0,
                cy + 22.0,
                cx + 12.0,
                cy + 14.0,
            );
            let _ = cr.stroke();
        }
        CompanionExpression::Focused => {
            cr.move_to(cx - 25.0, cy - 14.0);
            cr.line_to(cx - 11.0, cy - 8.0);
            cr.move_to(cx + 25.0, cy - 14.0);
            cr.line_to(cx + 11.0, cy - 8.0);
            let _ = cr.stroke();
        }
        CompanionExpression::Listening => {
            draw_eye_circle(cr, cx - 17.0, cy - 9.0, 6.0);
            draw_eye_circle(cr, cx + 17.0, cy - 9.0, 6.0);
            let radius = 4.0 + ((phase * 1.8).sin() + 1.0) * 1.2;
            cr.arc(cx + 1.0, cy + 15.0, radius, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();
        }
        CompanionExpression::Speaking => {
            draw_happy_eye(cr, cx - 17.0, cy - 8.0);
            draw_happy_eye(cr, cx + 17.0, cy - 8.0);
            let mouth = 5.0 + ((phase * 2.4).sin() + 1.0) * 3.0;
            cr.arc(cx, cy + 14.0, mouth, 0.15, std::f64::consts::PI - 0.15);
            let _ = cr.stroke();
        }
    }
}

fn draw_eye_circle(cr: &gtk::cairo::Context, x: f64, y: f64, radius: f64) {
    cr.arc(x, y, radius, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();
}

fn draw_happy_eye(cr: &gtk::cairo::Context, x: f64, y: f64) {
    cr.arc(
        x,
        y + 5.0,
        8.0,
        std::f64::consts::PI + 0.35,
        std::f64::consts::TAU - 0.35,
    );
    let _ = cr.stroke();
}

fn expression_from_state(state: &CompanionPresentationState) -> CompanionExpression {
    if matches!(state.runtime_mode, RuntimeMode::Disabled) {
        return CompanionExpression::Neutral;
    }

    if state.latest_diagnostic.as_ref().is_some_and(|diagnostic| {
        matches!(
            diagnostic.level,
            DiagnosticLevel::Warning | DiagnosticLevel::Error
        )
    }) {
        return CompanionExpression::Focused;
    }

    if state.pending_approval_count() > 0 {
        return CompanionExpression::Listening;
    }

    if state
        .tracked_work
        .iter()
        .any(|work| !work.terminal && work.cooperation_pending)
    {
        return CompanionExpression::Focused;
    }

    if state.tracked_work.iter().any(|work| !work.terminal) {
        return CompanionExpression::Thinking;
    }

    if matches!(state.runtime_mode, RuntimeMode::GameMode) {
        return CompanionExpression::Focused;
    }

    CompanionExpression::Happy
}

#[derive(Clone)]
struct StatusCard {
    card: GtkBox,
    mode_label: Label,
    detail_label: Label,
}

fn build_status_card(state: &CompanionPresentationState) -> StatusCard {
    let card = GtkBox::new(Orientation::Vertical, 2);
    card.add_css_class("status-card");

    let title = Label::new(Some("LYCHNOS"));
    title.add_css_class("lychnos-title");

    let mode_label = Label::new(None);
    mode_label.add_css_class("mode-label");

    let detail_label = Label::new(None);
    detail_label.add_css_class("detail-label");
    detail_label.set_max_width_chars(34);
    detail_label.set_wrap(true);

    card.append(&title);
    card.append(&mode_label);
    card.append(&detail_label);

    let widgets = StatusCard {
        card,
        mode_label,
        detail_label,
    };
    update_status_card(&widgets, state);
    widgets
}

fn update_status_card(widgets: &StatusCard, state: &CompanionPresentationState) {
    widgets.mode_label.set_label(mode_label(state.runtime_mode));

    widgets.detail_label.set_label(&status_detail(state));
}

fn status_detail(state: &CompanionPresentationState) -> String {
    if let Some(approval) = state.pending_approvals.first() {
        format!("Approval needed · {}", approval.reason)
    } else if let Some(diagnostic) = state.latest_diagnostic.as_ref().filter(|diagnostic| {
        matches!(
            diagnostic.level,
            DiagnosticLevel::Warning | DiagnosticLevel::Error
        )
    }) {
        diagnostic.message.clone()
    } else if let Some(work) = state.tracked_work.iter().find(|work| !work.terminal) {
        format!("Working · {}", work.state.as_str())
    } else {
        match state.runtime_mode {
            RuntimeMode::Normal => "Standing by".to_string(),
            RuntimeMode::GameMode => "Keeping a low profile".to_string(),
            RuntimeMode::Disabled => "Runtime disabled".to_string(),
        }
    }
}

fn default_presentation_state() -> CompanionPresentationState {
    CompanionPresentationState {
        runtime_mode: RuntimeMode::Normal,
        diagnostics_enabled: true,
        pending_approvals: Vec::new(),
        tracked_work: Vec::new(),
        latest_diagnostic: None,
    }
}

fn presentation_snapshot_path() -> PathBuf {
    if let Ok(path) = std::env::var("LYCHNOS_PRESENTATION_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("lychnos/presentation-v1.json");
    }

    std::env::temp_dir().join("lychnos/presentation-v1.json")
}

fn load_presentation_snapshot() -> Option<CompanionPresentationState> {
    let contents = fs::read_to_string(presentation_snapshot_path()).ok()?;
    CompanionPresentationEnvelope::from_json(&contents)
        .ok()
        .map(|envelope| envelope.state)
}

fn install_live_presentation_updates(
    state: Rc<RefCell<CompanionPresentationState>>,
    body: &DrawingArea,
    status: &StatusCard,
) {
    let body = body.clone();
    let status = status.clone();

    gtk::glib::timeout_add_local(Duration::from_millis(250), move || {
        let Some(next_state) = load_presentation_snapshot() else {
            return gtk::glib::ControlFlow::Continue;
        };

        let changed = {
            let current = state.borrow();
            *current != next_state
        };

        if changed {
            *state.borrow_mut() = next_state;
            update_status_card(&status, &state.borrow());
            body.queue_draw();
        }

        gtk::glib::ControlFlow::Continue
    });
}

fn mode_accent(mode: RuntimeMode, alert: bool) -> (f64, f64, f64) {
    if alert {
        return (1.0, 0.22, 0.14);
    }

    match mode {
        RuntimeMode::Normal => (0.05, 0.86, 1.0),
        RuntimeMode::GameMode => (0.36, 0.58, 1.0),
        RuntimeMode::Disabled => (0.34, 0.40, 0.44),
    }
}

fn mode_label(mode: RuntimeMode) -> &'static str {
    match mode {
        RuntimeMode::Normal => "NORMAL · AWAKE",
        RuntimeMode::GameMode => "GAME MODE · QUIET",
        RuntimeMode::Disabled => "DISABLED · SAFE",
    }
}

fn install_css() {
    let css = gtk::CssProvider::new();
    css.load_from_data(
        r#"
        window { background: transparent; }
        .status-card {
            background: rgba(8, 15, 20, 0.88);
            border: 1px solid rgba(51, 210, 255, 0.26);
            border-radius: 14px;
            padding: 8px 14px;
        }
        .lychnos-title {
            color: #80e9ff;
            font-size: 10px;
            font-weight: 800;
            letter-spacing: 2px;
        }
        .mode-label {
            color: #e8fbff;
            font-size: 11px;
            font-weight: 700;
        }
        .detail-label {
            color: rgba(225, 244, 248, 0.74);
            font-size: 10px;
        }
        .lychnos-menu > contents {
            background: rgba(8, 15, 20, 0.97);
            border: 1px solid rgba(51, 210, 255, 0.30);
            border-radius: 12px;
            padding: 6px;
        }
        .lychnos-menu-item {
            min-width: 148px;
            padding: 7px 10px;
            border-radius: 8px;
        }
        "#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lychnos_core::{
        action::{ActionId, ActionImpact, ActionKind, ActionRisk, Capability},
        diagnostics::DiagnosticLevel,
        executor::MockRunningWorkState,
        presentation::{
            DiagnosticPresentation, PendingApprovalPresentation, TrackedWorkPresentation,
        },
    };

    fn base_state(mode: RuntimeMode) -> CompanionPresentationState {
        CompanionPresentationState {
            runtime_mode: mode,
            diagnostics_enabled: true,
            pending_approvals: Vec::new(),
            tracked_work: Vec::new(),
            latest_diagnostic: None,
        }
    }

    #[test]
    fn active_work_maps_to_thinking() {
        let mut state = base_state(RuntimeMode::Normal);
        state.tracked_work.push(TrackedWorkPresentation::new(
            ActionId::new("work"),
            MockRunningWorkState::Running,
        ));

        assert_eq!(expression_from_state(&state), CompanionExpression::Thinking);
        assert_eq!(status_detail(&state), "Working · running");
    }

    #[test]
    fn pending_approval_maps_to_listening_attention() {
        let mut state = base_state(RuntimeMode::Normal);
        state.pending_approvals.push(PendingApprovalPresentation {
            action_id: ActionId::new("approval"),
            kind: ActionKind::new("demo.write"),
            capability: Capability::new("demo.write"),
            impact: ActionImpact::StateChanging,
            risk: ActionRisk::Moderate,
            reason: "Change demo state".into(),
        });

        assert_eq!(
            expression_from_state(&state),
            CompanionExpression::Listening
        );
        assert_eq!(status_detail(&state), "Approval needed · Change demo state");
    }

    #[test]
    fn game_mode_maps_to_focused_quiet_state() {
        let state = base_state(RuntimeMode::GameMode);

        assert_eq!(expression_from_state(&state), CompanionExpression::Focused);
        assert_eq!(status_detail(&state), "Keeping a low profile");
    }

    #[test]
    fn disabled_maps_to_neutral_safe_state() {
        let state = base_state(RuntimeMode::Disabled);

        assert_eq!(expression_from_state(&state), CompanionExpression::Neutral);
        assert_eq!(status_detail(&state), "Runtime disabled");
    }

    #[test]
    fn warning_overrides_idle_expression() {
        let mut state = base_state(RuntimeMode::Normal);
        state.latest_diagnostic = Some(DiagnosticPresentation {
            level: DiagnosticLevel::Warning,
            component: "test".into(),
            message: "Something needs attention".into(),
        });

        assert_eq!(expression_from_state(&state), CompanionExpression::Focused);
        assert_eq!(status_detail(&state), "Something needs attention");
    }
}
