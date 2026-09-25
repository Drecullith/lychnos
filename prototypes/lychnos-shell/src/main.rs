use gtk::prelude::*;
use std::{cell::Cell, fs, path::PathBuf, rc::Rc, time::Duration};

use gtk::{
    Application, ApplicationWindow, Box as GtkBox, DrawingArea, GestureClick, GestureDrag, Label,
    Orientation,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use lychnos_core::{
    diagnostics::DiagnosticLevel,
    presentation::{
        CompanionPresentationState, DiagnosticPresentation, PendingApprovalPresentation,
    },
    runtime::RuntimeMode,
};

const APP_ID: &str = "org.lychnos.prototype.shell";
const DEFAULT_TOP_MARGIN: i32 = 36;
const DEFAULT_RIGHT_MARGIN: i32 = 42;

fn main() {
    let application = Application::builder().application_id(APP_ID).build();
    application.connect_activate(build_ui);
    application.run();
}

fn build_ui(app: &Application) {
    install_css();

    let state = demo_state_from_args();
    let body = build_body(&state);
    let status = build_status_card(&state);

    let layout = GtkBox::new(Orientation::Vertical, 8);
    layout.set_halign(gtk::Align::Center);
    layout.append(&body);
    layout.append(&status);
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

    install_shell_interactions(&window, &body, &status, top_margin, right_margin);

    window.present();
}

fn install_shell_interactions(
    window: &ApplicationWindow,
    body: &DrawingArea,
    status: &GtkBox,
    top_margin: i32,
    right_margin: i32,
) {
    let current_top = Rc::new(Cell::new(top_margin));
    let current_right = Rc::new(Cell::new(right_margin));
    let drag_start_top = Rc::new(Cell::new(top_margin));
    let drag_start_right = Rc::new(Cell::new(right_margin));

    let drag = GestureDrag::new();

    {
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let drag_start_top = Rc::clone(&drag_start_top);
        let drag_start_right = Rc::clone(&drag_start_right);

        drag.connect_drag_begin(move |_, _, _| {
            drag_start_top.set(current_top.get());
            drag_start_right.set(current_right.get());
        });
    }

    {
        let window = window.clone();
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);
        let drag_start_top = Rc::clone(&drag_start_top);
        let drag_start_right = Rc::clone(&drag_start_right);

        drag.connect_drag_update(move |_, offset_x, offset_y| {
            let top = (drag_start_top.get() + offset_y.round() as i32).max(0);
            let right = (drag_start_right.get() - offset_x.round() as i32).max(0);

            current_top.set(top);
            current_right.set(right);
            window.set_margin(Edge::Top, top);
            window.set_margin(Edge::Right, right);
        });
    }

    {
        let current_top = Rc::clone(&current_top);
        let current_right = Rc::clone(&current_right);

        drag.connect_drag_end(move |_, _, _| {
            save_shell_position(current_top.get(), current_right.get());
        });
    }

    body.add_controller(drag);

    let click = GestureClick::new();
    {
        let status = status.clone();
        click.connect_released(move |_, press_count, _, _| {
            if press_count == 2 {
                status.set_visible(!status.is_visible());
            }
        });
    }
    body.add_controller(click);
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

fn build_body(state: &CompanionPresentationState) -> DrawingArea {
    let area = DrawingArea::new();
    area.set_content_width(210);
    area.set_content_height(210);
    area.set_tooltip_text(Some("Drag to move · double-click to hide/show status"));

    let mode = state.runtime_mode;
    let approvals = state.pending_approval_count();
    let alert = state.latest_diagnostic.is_some();
    let phase = Rc::new(Cell::new(0.0_f64));
    let draw_phase = Rc::clone(&phase);

    area.set_draw_func(move |_, cr, width, height| {
        let w = f64::from(width);
        let h = f64::from(height);
        let cx = w / 2.0;
        let bob = draw_phase.get().sin() * 4.5;
        let pulse = (draw_phase.get() * 0.72).sin() * 0.05;
        let cy = h / 2.0 - 9.0 + bob;
        let shell_radius = w.min(h) * 0.31;
        let face_radius = shell_radius * 0.69;
        let (r, g, b) = mode_accent(mode, alert);

        // Canonical hover ring.
        draw_ellipse(
            cr,
            cx,
            cy + shell_radius + 25.0,
            shell_radius * 0.72,
            8.5,
            (r, g, b, 0.17 + pulse),
            0.0,
        );
        draw_ellipse(
            cr,
            cx,
            cy + shell_radius + 25.0,
            shell_radius * 0.62,
            5.0,
            (r, g, b, 0.46 + pulse),
            1.8,
        );

        // Six armor petals around the glossy face, echoing the locked Lychnos body.
        for angle in [-1.57_f64, -0.53, 0.52, 1.57, 2.62, 3.67] {
            draw_shell_petal(cr, cx, cy, shell_radius, angle, (r, g, b), 0.70 + pulse);
        }

        // Inner structural ring.
        cr.set_source_rgba(0.035, 0.055, 0.072, 0.98);
        cr.arc(cx, cy, shell_radius * 0.80, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        cr.set_source_rgba(r, g, b, 0.18);
        cr.set_line_width(2.0);
        cr.arc(cx, cy, shell_radius * 0.79, -2.85, -0.30);
        let _ = cr.stroke();

        // Glossy black face disc.
        let face_gradient = gtk::cairo::RadialGradient::new(
            cx - face_radius * 0.24,
            cy - face_radius * 0.28,
            face_radius * 0.08,
            cx,
            cy,
            face_radius,
        );
        face_gradient.add_color_stop_rgb(0.0, 0.075, 0.095, 0.11);
        face_gradient.add_color_stop_rgb(0.48, 0.018, 0.025, 0.034);
        face_gradient.add_color_stop_rgb(1.0, 0.002, 0.005, 0.009);
        let _ = cr.set_source(&face_gradient);
        cr.arc(cx, cy, face_radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        // Glass highlight.
        cr.set_source_rgba(0.55, 0.78, 0.92, 0.11);
        cr.set_line_width(4.0);
        cr.arc(cx - 3.0, cy - 2.0, face_radius - 5.0, 3.55, 5.08);
        let _ = cr.stroke();

        draw_face_expression(cr, cx, cy, mode, alert, (r, g, b));

        // Approval notification pip.
        if approvals > 0 {
            cr.set_source_rgba(1.0, 0.56, 0.16, 0.98);
            cr.arc(
                cx + shell_radius * 0.78,
                cy - shell_radius * 0.68,
                8.0,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.fill();

            cr.set_source_rgba(1.0, 0.78, 0.38, 0.36);
            cr.set_line_width(4.0);
            cr.arc(
                cx + shell_radius * 0.78,
                cy - shell_radius * 0.68,
                11.0,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.stroke();
        }
    });

    let animation_area = area.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(40), move || {
        let next = phase.get() + 0.08;
        phase.set(if next > std::f64::consts::TAU {
            next - std::f64::consts::TAU
        } else {
            next
        });
        animation_area.queue_draw();
        gtk::glib::ControlFlow::Continue
    });

    area
}

fn build_status_card(state: &CompanionPresentationState) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 2);
    card.add_css_class("status-card");

    let title = Label::new(Some("LYCHNOS"));
    title.add_css_class("lychnos-title");

    let mode = Label::new(Some(mode_label(state.runtime_mode)));
    mode.add_css_class("mode-label");

    let detail = if state.pending_approval_count() > 0 {
        format!("{} approval waiting", state.pending_approval_count())
    } else if let Some(diagnostic) = &state.latest_diagnostic {
        diagnostic.message.clone()
    } else {
        "Standing by".to_string()
    };

    let detail_label = Label::new(Some(&detail));
    detail_label.add_css_class("detail-label");
    detail_label.set_max_width_chars(34);
    detail_label.set_wrap(true);

    card.append(&title);
    card.append(&mode);
    card.append(&detail_label);
    card
}

fn demo_state_from_args() -> CompanionPresentationState {
    let requested = std::env::var("LYCHNOS_DEMO_STATE").unwrap_or_else(|_| "normal".into());

    let runtime_mode = match requested.as_str() {
        "game" | "gamemode" => RuntimeMode::GameMode,
        "disabled" => RuntimeMode::Disabled,
        _ => RuntimeMode::Normal,
    };

    let pending_approvals = if requested == "approval" {
        vec![PendingApprovalPresentation {
            action_id: lychnos_core::action::ActionId::new("demo-approval"),
            kind: lychnos_core::action::ActionKind::new("prototype.preview"),
            capability: lychnos_core::action::Capability::new("prototype.preview"),
            impact: lychnos_core::action::ActionImpact::StateChanging,
            risk: lychnos_core::action::ActionRisk::Low,
            reason: "Prototype approval indicator".into(),
        }]
    } else {
        Vec::new()
    };

    let latest_diagnostic = (requested == "alert").then(|| DiagnosticPresentation {
        level: DiagnosticLevel::Warning,
        component: "prototype".into(),
        message: "Terminal 3 hit a snag…".into(),
    });

    CompanionPresentationState {
        runtime_mode,
        diagnostics_enabled: true,
        pending_approvals,
        tracked_work: Vec::new(),
        latest_diagnostic,
    }
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

fn draw_shell_petal(
    cr: &gtk::cairo::Context,
    cx: f64,
    cy: f64,
    shell_radius: f64,
    angle: f64,
    accent: (f64, f64, f64),
    glow_alpha: f64,
) {
    let radial = shell_radius * 0.80;
    let px = cx + angle.cos() * radial;
    let py = cy + angle.sin() * radial;
    let petal_radius = shell_radius * 0.43;
    let (r, g, b) = accent;

    if cr.save().is_err() {
        return;
    }

    cr.translate(px, py);
    cr.rotate(angle + std::f64::consts::FRAC_PI_2);
    cr.scale(1.0, 0.52);

    let gradient =
        gtk::cairo::LinearGradient::new(-petal_radius, -petal_radius, petal_radius, petal_radius);
    gradient.add_color_stop_rgb(0.0, 0.15, 0.18, 0.23);
    gradient.add_color_stop_rgb(0.42, 0.07, 0.09, 0.12);
    gradient.add_color_stop_rgb(1.0, 0.018, 0.026, 0.035);
    let _ = cr.set_source(&gradient);
    cr.arc(0.0, 0.0, petal_radius, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();

    cr.set_source_rgba(0.32, 0.42, 0.52, 0.54);
    cr.set_line_width(2.4);
    cr.arc(0.0, 0.0, petal_radius - 1.4, 0.0, std::f64::consts::TAU);
    let _ = cr.stroke();

    cr.set_source_rgba(r, g, b, glow_alpha.clamp(0.0, 1.0));
    cr.set_line_width(3.4);
    cr.arc(0.0, 0.0, petal_radius - 4.0, -2.35, -0.42);
    let _ = cr.stroke();

    let _ = cr.restore();
}

fn draw_ellipse(
    cr: &gtk::cairo::Context,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    rgba: (f64, f64, f64, f64),
    line_width: f64,
) {
    if cr.save().is_err() {
        return;
    }

    cr.translate(cx, cy);
    cr.scale(rx, ry);
    cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    let _ = cr.restore();

    cr.set_source_rgba(rgba.0, rgba.1, rgba.2, rgba.3.clamp(0.0, 1.0));
    if line_width > 0.0 {
        cr.set_line_width(line_width);
        let _ = cr.stroke();
    } else {
        let _ = cr.fill();
    }
}

fn draw_face_expression(
    cr: &gtk::cairo::Context,
    cx: f64,
    cy: f64,
    mode: RuntimeMode,
    alert: bool,
    accent: (f64, f64, f64),
) {
    let (r, g, b) = accent;
    cr.set_source_rgb(r, g, b);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_width(5.4);

    if alert {
        cr.move_to(cx - 28.0, cy - 10.0);
        cr.line_to(cx - 12.0, cy - 4.0);
        let _ = cr.stroke();

        cr.move_to(cx + 12.0, cy - 4.0);
        cr.line_to(cx + 28.0, cy - 10.0);
        let _ = cr.stroke();
        return;
    }

    match mode {
        RuntimeMode::Normal => {
            cr.move_to(cx - 29.0, cy - 4.0);
            cr.curve_to(
                cx - 25.0,
                cy - 16.0,
                cx - 13.0,
                cy - 16.0,
                cx - 9.0,
                cy - 4.0,
            );
            let _ = cr.stroke();

            cr.move_to(cx + 9.0, cy - 4.0);
            cr.curve_to(
                cx + 13.0,
                cy - 16.0,
                cx + 25.0,
                cy - 16.0,
                cx + 29.0,
                cy - 4.0,
            );
            let _ = cr.stroke();

            cr.set_line_width(2.6);
            cr.move_to(cx - 8.0, cy + 16.0);
            cr.curve_to(
                cx - 3.0,
                cy + 20.0,
                cx + 3.0,
                cy + 20.0,
                cx + 8.0,
                cy + 16.0,
            );
            let _ = cr.stroke();
        }
        RuntimeMode::GameMode => {
            cr.move_to(cx - 27.0, cy - 2.0);
            cr.line_to(cx - 11.0, cy - 2.0);
            let _ = cr.stroke();

            cr.move_to(cx + 11.0, cy - 2.0);
            cr.line_to(cx + 27.0, cy - 2.0);
            let _ = cr.stroke();
        }
        RuntimeMode::Disabled => {
            cr.set_source_rgba(r, g, b, 0.62);
            cr.set_line_width(4.0);
            cr.move_to(cx - 24.0, cy - 1.0);
            cr.line_to(cx - 14.0, cy - 1.0);
            let _ = cr.stroke();

            cr.move_to(cx + 14.0, cy - 1.0);
            cr.line_to(cx + 24.0, cy - 1.0);
            let _ = cr.stroke();
        }
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
