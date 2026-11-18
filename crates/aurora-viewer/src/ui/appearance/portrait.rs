//! The outfit's auto portrait window and the square photo mode's banner
//! and helpers (Fider #447; the rules are `snapshot::portrait`, the app's
//! side `app::outfit_portrait`). The window: the live preview, square, with
//! its thirds and corners, the choices (framing, pose, angle, backdrop,
//! light), then Take, the photo to confirm, Use or Retake. An outfit not
//! worn is offered to be worn first; once the photo is over, the previous
//! outfit is offered back. Nothing is worn or taken off without a click.
//! The paid HD picture (`hd_choice`) can be chosen with the photo shown:
//! its price comes first, and Use asks to confirm it before it is paid.

use super::super::{icons, widgets};
use crate::snapshot::portrait::{Angle, Backdrop, Framing, HdOffer, HdSide, Lighting, PortraitSettings, Pose, Ratio};
use crate::theme::Palette;
use crate::ui::fonts;
use egui::{Align2, Rect, RichText, Sense, Vec2};

/// The auto portrait's window.
pub const WINDOW: &str = "outfit_portrait";
/// The previous outfit's question, once the photo is over.
pub const DONE_WINDOW: &str = "outfit_portrait_done";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Shoot,
    Use,
    Retake,
    /// Cancel, the window's X or the photo mode's: the photo part ends.
    Cancel,
    /// Wear the outfit for the photo.
    Wear,
    /// Put the outfit worn before back on.
    PutBack,
    /// Keep the outfit worn for the photo.
    Keep,
    /// The same outfit in the square photo mode.
    PhotoMode,
    /// The photo mode's helpers.
    Reframe,
    Pose,
    Studio,
    /// Clear the camera adjustment for the current framing choices.
    ResetCamera,
    SaveArrangement,
    NewArrangement,
    SelectArrangement(usize),
    DeleteArrangement,
    RenameArrangement(usize, String),
    /// The HD picture's price confirmed: the photo is used, and sent.
    ConfirmHd,
    /// Back to the photo, nothing paid.
    CancelHd,
}

/// The paid HD picture, as the window offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdView {
    /// Its price for the side chosen, and whether the balance pays it.
    pub offer: HdOffer,
    /// Another one is still on its way.
    pub busy: bool,
    /// Use asked to confirm this price.
    pub confirm: Option<i32>,
    /// The outfit has an HD picture already (it goes to the Trash).
    pub has_one: bool,
}

/// What the window shows.
pub struct View<'a> {
    pub name: &'a str,
    pub preview: Option<&'a egui::TextureHandle>,
    /// The photo awaiting Use.
    pub shot: Option<&'a egui::TextureHandle>,
    /// The outfit's picture now.
    pub current: Option<&'a egui::TextureHandle>,
    pub worn: bool,
    /// Worn from here: the name of the outfit worn before (None inside:
    /// not known).
    pub previous: Option<Option<&'a str>>,
    /// The current outfit has unsaved changes.
    pub dirty: bool,
    pub shooting: bool,
    /// The photo part is over: the previous outfit's question.
    pub over: bool,
    pub saved: bool,
    /// The photo mode's helpers that are on.
    pub pose: bool,
    pub studio: bool,
    pub hd: HdView,
}

const FRAMINGS: [Framing; 3] = [Framing::Body, Framing::Bust, Framing::Face];
const RATIOS: [Ratio; 2] = [Ratio::Square, Ratio::Portrait];
const POSES: [Pose; 3] = [Pose::A, Pose::ArmsDown, Pose::Current];
const ANGLES: [Angle; 3] = [Angle::Front, Angle::ThreeQuarter, Angle::Profile];
const BACKDROPS: [Backdrop; 4] = [Backdrop::Dark, Backdrop::Grey, Backdrop::Light, Backdrop::Scene];
const LIGHTINGS: [Lighting; 2] = [Lighting::Studio, Lighting::Region];
const HD_SIDES: [HdSide; 2] = [HdSide::Side512, HdSide::Side1024];

/// The auto portrait's window, or once its photo is over, the previous
/// outfit's question; `open` is cleared by its X.
pub fn window(ctx: &egui::Context, p: &Palette, settings: &mut PortraitSettings, view: &View, open: &mut bool) -> Vec<Action> {
    let mut actions = Vec::new();
    let title = aurora_i18n::tr!("ui-appearance-portrait-title", outfit = view.name).to_string();
    if view.over {
        widgets::Floater::new(DONE_WINDOW, title, egui::pos2(160.0, 140.0), egui::vec2(420.0, 160.0))
            .auto_height()
            .show(ctx, p, open, |ui| done(ui, p, view, &mut actions));
        return actions;
    }
    widgets::Floater::new(WINDOW, title, egui::pos2(120.0, 80.0), egui::vec2(780.0, 600.0))
        .steady_size()
        .show(ctx, p, open, |ui| {
            let (mut above, footer) = widgets::above_footer(ui);
            body(&mut above, p, settings, view, &mut actions);
            footer.show(ui, |ui| footer_ui(ui, p, view, &mut actions));
        });
    actions
}

/// The preview and its notes on the left, the choices on the right. The
/// frame retains its selected ratio and fits within the window's available
/// height; notes continue below it in the scroll.
fn body(ui: &mut egui::Ui, p: &Palette, settings: &mut PortraitSettings, view: &View, actions: &mut Vec<Action>) {
    let room = ui.available_rect_before_wrap();
    let frame_size = frame_size(room.size(), settings.ratio.aspect());
    let (width, height) = (frame_size.x, frame_size.y);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        ui.vertical(|ui| {
            ui.set_width(width);
            egui::ScrollArea::vertical()
                .id_salt("outfit_portrait_preview_notes")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 10.0;
                    let (frame, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
                    preview(ui, p, frame, view);
                    not_worn(ui, p, view, actions);
                    note(ui, p, "info-circle-filled", aurora_i18n::tr!("ui-appearance-portrait-local"));
                    if let Some(t) = view.current {
                        current(ui, p, t);
                    }
                });
        });
        ui.vertical(|ui| {
            // (scrolled when a large text or a short window leaves no room)
            egui::ScrollArea::vertical()
                .id_salt("outfit_portrait_choices")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let shot = view.shot.is_some() || view.shooting;
                    actions.extend(ui.add_enabled_ui(!shot, |ui| choices_ui(ui, p, settings)).inner);
                    ui.add_space(6.0);
                    // (chosen with the photo shown too: it is drawn at the
                    // largest side)
                    ui.add_enabled_ui(!view.shooting, |ui| hd_choice(ui, p, settings, &view.hd));
                });
        });
    });
}

fn frame_size(available: Vec2, aspect: f32) -> Vec2 {
    let max_width = (available.x * 0.54).max(120.0).min(available.x);
    let height = (max_width / aspect).min(available.y.max(1.0));
    Vec2::new(height * aspect, height)
}

pub(super) fn choices_ui(ui: &mut egui::Ui, p: &Palette, s: &mut PortraitSettings) -> Vec<Action> {
    let mut actions = Vec::new();
    saved_arrangements(ui, p, s, &mut actions);
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-framing"),
        &mut s.framing,
        &FRAMINGS,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-body"),
            aurora_i18n::tr!("ui-appearance-portrait-bust"),
            aurora_i18n::tr!("ui-appearance-portrait-face"),
        ],
    );
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-ratio"),
        &mut s.ratio,
        &RATIOS,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-ratio-square"),
            aurora_i18n::tr!("ui-appearance-portrait-ratio-portrait"),
        ],
    );
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-pose"),
        &mut s.pose,
        &POSES,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-pose-a"),
            aurora_i18n::tr!("ui-appearance-portrait-arms-down"),
            aurora_i18n::tr!("ui-appearance-portrait-pose-current"),
        ],
    );
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-angle"),
        &mut s.angle,
        &ANGLES,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-front"),
            aurora_i18n::tr!("ui-appearance-portrait-three-quarter"),
            aurora_i18n::tr!("ui-appearance-portrait-profile"),
        ],
    );
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-backdrop"),
        &mut s.backdrop,
        &BACKDROPS,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-dark"),
            aurora_i18n::tr!("ui-appearance-portrait-grey"),
            aurora_i18n::tr!("ui-appearance-portrait-light"),
            aurora_i18n::tr!("ui-appearance-portrait-scene"),
        ],
    );
    choice(
        ui,
        p,
        aurora_i18n::tr!("ui-appearance-portrait-lighting"),
        &mut s.lighting,
        &LIGHTINGS,
        &[
            aurora_i18n::tr!("ui-appearance-portrait-studio"),
            aurora_i18n::tr!("ui-appearance-portrait-region"),
        ],
    );
    ui.add_space(6.0);
    // (the backdrop alone already shows our avatar alone)
    ui.add_enabled_ui(s.backdrop == Backdrop::Scene, |ui| {
        widgets::checkbox(ui, p, &mut s.hide_others, aurora_i18n::tr!("ui-appearance-portrait-hide-others"));
    });
    widgets::checkbox(ui, p, &mut s.keep, aurora_i18n::tr!("ui-appearance-portrait-keep"));
    actions
}

/// Explicitly saved views can be recalled even for the current choices.
fn saved_arrangements(ui: &mut egui::Ui, p: &Palette, s: &PortraitSettings, actions: &mut Vec<Action>) {
    ui.label(
        RichText::new(aurora_i18n::tr!("ui-appearance-portrait-arrangements"))
            .size(12.0)
            .family(fonts::semibold(ui.ctx()))
            .color(ui.visuals().weak_text_color()),
    );
    let selected = s
        .selected_arrangement
        .filter(|&i| s.arrangements.get(i).is_some_and(Option::is_some));
    let new_label = aurora_i18n::tr!("ui-appearance-portrait-new-arrangement");
    let font = widgets::control_font(ui, 12.0);
    let label_width = ui.fonts_mut(|fonts| fonts.layout_no_wrap(new_label.to_owned(), font, p.ink).size().x);
    let new_width = (label_width + 2.0 * ui.spacing().button_padding.x).max(ui.spacing().interact_size.x);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        egui::ComboBox::from_id_salt("outfit_portrait_saved_arrangements")
            .selected_text("")
            .width(new_width)
            .show_ui(ui, |ui| {
                for i in 0..s.arrangements.len() {
                    if let Some(name) = s.arrangement_name(i)
                        && ui.selectable_label(selected == Some(i), name).clicked()
                    {
                        actions.push(Action::SelectArrangement(i));
                    }
                }
            });
        if let Some(index) = selected {
            let id = ui.scope_id().with(("portrait_arrangement_name", index));
            let stored = s.arrangement_name(index).unwrap_or_default();
            let mut name = ui.ctx().data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| stored.clone());
            let field_width = ui.available_width().max(60.0);
            let response = ui.add_sized(
                Vec2::new(field_width, 28.0),
                egui::TextEdit::singleline(&mut name)
                    .id(id)
                    .char_limit(32)
                    .align(egui::Align2::CENTER_CENTER),
            );
            if response.lost_focus() {
                if name.trim() != stored {
                    actions.push(Action::RenameArrangement(index, name.clone()));
                }
                ui.ctx().data_mut(|d| d.remove::<String>(id));
            } else if response.has_focus() {
                ui.ctx().data_mut(|d| d.insert_temp(id, name));
            } else {
                ui.ctx().data_mut(|d| d.remove::<String>(id));
            }
        } else {
            let field_width = ui.available_width().max(60.0);
            ui.add_enabled_ui(false, |ui| {
                ui.add_sized(
                    Vec2::new(field_width, 28.0),
                    egui::TextEdit::singleline(&mut aurora_i18n::tr!("ui-appearance-portrait-no-arrangement").to_string())
                        .align(egui::Align2::CENTER_CENTER),
                );
            });
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_enabled_ui(s.arrangements.iter().any(Option::is_none), |ui| {
            if widgets::flat_button_sized(ui, p, new_label, Vec2::new(new_width, ui.spacing().interact_size.y)).clicked() {
                actions.push(Action::NewArrangement);
            }
        });
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if widgets::flat_button(ui, p, aurora_i18n::tr!("ui-appearance-portrait-save-arrangement")).clicked() {
                actions.push(Action::SaveArrangement);
            }
        });
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if widgets::flat_button(ui, p, aurora_i18n::tr!("ui-appearance-portrait-delete-arrangement")).clicked() {
                actions.push(Action::DeleteArrangement);
            }
        });
        if widgets::flat_button(ui, p, aurora_i18n::tr!("ui-appearance-portrait-reset-camera")).clicked() {
            actions.push(Action::ResetCamera);
        }
    });
}

/// The optional larger picture: upload it for the shown price, save it as a
/// PNG locally, or keep only the free thumbnail.
fn hd_choice(ui: &mut egui::Ui, p: &Palette, s: &mut PortraitSettings, hd: &HdView) {
    ui.label(
        RichText::new(aurora_i18n::tr!("ui-appearance-portrait-hd-title"))
            .size(12.0)
            .family(fonts::semibold(ui.ctx()))
            .color(p.muted),
    );
    let upload_ready = matches!(hd.offer, HdOffer::Ready(_)) && !hd.busy && hd.confirm.is_none();
    let upload = match hd.offer {
        HdOffer::Ready(cost) | HdOffer::Short(cost) => aurora_i18n::tr!("ui-appearance-portrait-hd", cost = cost).to_string(),
        HdOffer::Unknown => aurora_i18n::tr!("ui-appearance-portrait-hd-no-price").to_string(),
    };
    ui.add_enabled_ui(upload_ready, |ui| {
        if widgets::radio(ui, p, s.hd && !s.hd_local, upload).clicked() {
            s.hd = true;
            s.hd_local = false;
        }
    });
    if widgets::radio(ui, p, s.hd_local, aurora_i18n::tr!("ui-appearance-portrait-hd-local")).clicked() {
        s.hd = false;
        s.hd_local = true;
    }
    if widgets::radio(ui, p, !s.hd && !s.hd_local, aurora_i18n::tr!("ui-appearance-portrait-hd-none")).clicked() {
        s.hd = false;
        s.hd_local = false;
    }
    let on = s.hd || s.hd_local;
    ui.add_enabled_ui(on, |ui| {
        let mut selected = HD_SIDES.iter().position(|v| *v == s.hd_side).unwrap_or(0);
        let small = s.ratio.dimensions(HdSide::Side512.side());
        let large = s.ratio.dimensions(HdSide::Side1024.side());
        let small_label = format!("{} × {}", small.0, small.1);
        let large_label = format!("{} × {}", large.0, large.1);
        let options = [(small_label.as_str(), false), (large_label.as_str(), false)];
        if widgets::segmented_size(ui, &options).x <= ui.available_width() {
            if widgets::segmented(ui, p, &mut selected, &options) {
                s.hd_side = HD_SIDES[selected];
            }
        } else {
            for (i, (label, _)) in options.iter().enumerate() {
                if widgets::radio(ui, p, selected == i, *label).clicked() {
                    s.hd_side = HD_SIDES[i];
                }
            }
        }
    });
    let reason = match hd.offer {
        HdOffer::Short(cost) if s.hd => Some(aurora_i18n::tr!("ui-appearance-portrait-hd-short", cost = cost).to_string()),
        HdOffer::Unknown if s.hd => Some(aurora_i18n::tr!("ui-appearance-portrait-hd-unknown").to_string()),
        HdOffer::Ready(_) if hd.busy => Some(aurora_i18n::tr!("ui-appearance-portrait-hd-busy").to_string()),
        _ => None,
    };
    match reason {
        Some(reason) => note(ui, p, "alert-triangle-filled", &reason),
        None if s.hd_local => note(
            ui,
            p,
            "info-circle-filled",
            aurora_i18n::tr!("ui-appearance-portrait-hd-local-about"),
        ),
        None if s.hd && hd.has_one => note(ui, p, "info-circle-filled", aurora_i18n::tr!("ui-appearance-portrait-hd-replaces")),
        None if s.hd => note(ui, p, "info-circle-filled", aurora_i18n::tr!("ui-appearance-portrait-hd-about")),
        None => {}
    }
}

/// The HD box: its title, the box (`enabled`; shown ticked only when the
/// price can be paid, `payable`) and the side under it. True when ticked.
/// Shared with the batch portraits' start (`batch`).
pub(super) fn hd_box(
    ui: &mut egui::Ui,
    p: &Palette,
    s: &mut PortraitSettings,
    title: &str,
    label: String,
    enabled: bool,
    payable: bool,
) -> bool {
    ui.label(RichText::new(title).size(12.0).family(fonts::semibold(ui.ctx())).color(p.muted));
    let ready = enabled;
    let mut on = s.hd && payable;
    ui.add_enabled_ui(ready, |ui| {
        if widgets::checkbox(ui, p, &mut on, label).changed() {
            s.hd = on;
            s.hd_local = false;
        }
    });
    ui.add_enabled_ui(ready && on, |ui| {
        let mut selected = HD_SIDES.iter().position(|v| *v == s.hd_side).unwrap_or(0);
        let small = s.ratio.dimensions(HdSide::Side512.side());
        let large = s.ratio.dimensions(HdSide::Side1024.side());
        let small_label = format!("{} × {}", small.0, small.1);
        let large_label = format!("{} × {}", large.0, large.1);
        let options = [(small_label.as_str(), false), (large_label.as_str(), false)];
        // (side by side, or one under the other in a narrow panel)
        if widgets::segmented_size(ui, &options).x <= ui.available_width() {
            if widgets::segmented(ui, p, &mut selected, &options) {
                s.hd_side = HD_SIDES[selected];
            }
        } else {
            for (i, (label, _)) in options.iter().enumerate() {
                if widgets::radio(ui, p, selected == i, *label).clicked() {
                    s.hd_side = HD_SIDES[i];
                }
            }
        }
    });
    on
}

/// The HD picture's price to confirm before Use pays it, like "Save to
/// inventory" (LLPanelSnapshotInventory::onSend): Back, or Pay and use;
/// one under the other in the photo mode's narrow panel (`stacked`).
fn hd_confirm(ui: &mut egui::Ui, p: &Palette, cost: i32, stacked: bool, actions: &mut Vec<Action>) {
    let text = aurora_i18n::tr!("ui-appearance-portrait-hd-confirm", cost = cost);
    let pay = aurora_i18n::tr!("ui-appearance-portrait-hd-pay", cost = cost).to_string();
    match pay_or_back(ui, p, &text, None, &pay, stacked) {
        Some(true) => actions.push(Action::ConfirmHd),
        Some(false) => actions.push(Action::CancelHd),
        None => {}
    }
}

/// A price to confirm: its text (and the balance, when known), then Back
/// or Pay; one under the other when `stacked` or when they do not fit side
/// by side. Some(true): paid, Some(false): back. Shared with the batch
/// portraits' start (`batch`).
pub(super) fn pay_or_back(ui: &mut egui::Ui, p: &Palette, text: &str, balance: Option<&str>, pay: &str, stacked: bool) -> Option<bool> {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(p.ink)).wrap());
    if let Some(balance) = balance {
        ui.add(egui::Label::new(RichText::new(balance).size(12.0).color(p.muted)).wrap());
    }
    let back = aurora_i18n::tr!("ui-appearance-portrait-hd-back");
    let fit = widgets::action_button_fit(ui, "check", pay).max(widgets::action_button_fit(ui, "arrow-back-up", back));
    if stacked || fit * 2.0 + ui.spacing().item_spacing.x > ui.available_width() {
        let size = Vec2::new(ui.available_width(), 34.0);
        if widgets::action_button(ui, p, "check", pay, widgets::ActionKind::Primary, size).clicked() {
            return Some(true);
        }
        if widgets::action_button(ui, p, "arrow-back-up", back, widgets::ActionKind::Flat, size).clicked() {
            return Some(false);
        }
        return None;
    }
    let buttons = [
        widgets::DialogButton {
            icon: "arrow-back-up",
            label: back,
            kind: widgets::ActionKind::Flat,
            enabled: true,
        },
        widgets::DialogButton {
            icon: "check",
            label: pay,
            kind: widgets::ActionKind::Primary,
            enabled: true,
        },
    ];
    widgets::dialog_buttons(ui, p, &buttons, 32.0).map(|i| i == 1)
}

/// One choice: its title, then its options side by side, or one under the
/// other when they do not fit (a narrow window, a large text).
fn choice<T: Copy + PartialEq>(ui: &mut egui::Ui, p: &Palette, title: &str, value: &mut T, all: &[T], labels: &[&str]) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(12.0).family(fonts::semibold(ui.ctx())).color(p.muted));
    let mut selected = all.iter().position(|v| v == value).unwrap_or(0);
    let options: Vec<(&str, bool)> = labels.iter().map(|l| (*l, false)).collect();
    let changed = if widgets::segmented_size(ui, &options).x <= ui.available_width() {
        widgets::segmented(ui, p, &mut selected, &options)
    } else {
        let mut changed = false;
        for (i, label) in labels.iter().enumerate() {
            if widgets::radio(ui, p, selected == i, *label).clicked() && selected != i {
                selected = i;
                changed = true;
            }
        }
        changed
    };
    if changed {
        *value = all[selected];
    }
}

/// The preview, or the photo to confirm, with the thirds and framing marks.
fn preview(ui: &egui::Ui, p: &Palette, frame: Rect, view: &View) {
    let painter = ui.painter_at(frame);
    let radius = ui.visuals().window_corner_radius;
    painter.rect_filled(frame, radius, p.bar);
    let image = view.shot.or(view.preview);
    if let Some(t) = image {
        let (image_rect, uv) = super::preview_rects(frame.shrink(1.0), t.size_vec2(), false);
        painter.image(t.id(), image_rect, uv, widgets::PICTURE_TINT);
    } else {
        painter.text(
            frame.center(),
            Align2::CENTER_CENTER,
            aurora_i18n::tr!("ui-appearance-portrait-preparing"),
            egui::FontId::proportional(13.0),
            p.muted,
        );
    }
    let line = egui::Stroke::new(1.0, p.ink.gamma_multiply(0.16));
    for i in 1..3 {
        let f = i as f32 / 3.0;
        let x = frame.left() + frame.width() * f;
        let y = frame.top() + frame.height() * f;
        painter.line_segment([egui::pos2(x, frame.top()), egui::pos2(x, frame.bottom())], line);
        painter.line_segment([egui::pos2(frame.left(), y), egui::pos2(frame.right(), y)], line);
    }
    let corner = egui::Stroke::new(2.0, p.ink);
    let (inset, arm) = (12.0, 22.0);
    let r = frame.shrink(inset);
    for (at, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([at, at + Vec2::new(arm * dx, 0.0)], corner);
        painter.line_segment([at, at + Vec2::new(0.0, arm * dy)], corner);
    }
}

/// An icon and a muted, wrapped line.
pub(super) fn note(ui: &mut egui::Ui, p: &Palette, icon: &str, text: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if let Some(t) = icons::global(icon) {
            ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::splat(16.0)).tint(p.muted));
        }
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(p.muted)).wrap());
    });
}

/// The outfit not worn: the offer to wear it, and what putting the
/// current one back would lose.
fn not_worn(ui: &mut egui::Ui, p: &Palette, view: &View, actions: &mut Vec<Action>) {
    if view.worn {
        return;
    }
    if view.previous.is_some() {
        note(ui, p, "hanger-2-filled", aurora_i18n::tr!("ui-appearance-portrait-wearing"));
        return;
    }
    egui::Frame::new()
        .fill(p.field)
        .corner_radius(ui.visuals().window_corner_radius)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            note(ui, p, "alert-triangle-filled", aurora_i18n::tr!("ui-appearance-portrait-not-worn"));
            if view.dirty {
                note(ui, p, "info-circle-filled", aurora_i18n::tr!("ui-appearance-portrait-unsaved"));
            }
            let label = aurora_i18n::tr!("ui-appearance-portrait-wear");
            let width = widgets::action_button_fit(ui, "hanger-2-filled", label);
            if widgets::action_button(
                ui,
                p,
                "hanger-2-filled",
                label,
                widgets::ActionKind::Primary,
                Vec2::new(width, 30.0),
            )
            .clicked()
            {
                actions.push(Action::Wear);
            }
        });
}

/// The outfit's picture now, which the photo replaces.
fn current(ui: &mut egui::Ui, p: &Palette, t: &egui::TextureHandle) {
    egui::Frame::new()
        .fill(p.field)
        .corner_radius(ui.visuals().window_corner_radius)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.add(
                    egui::Image::new(t)
                        .fit_to_exact_size(Vec2::splat(54.0))
                        .corner_radius(ui.visuals().window_corner_radius),
                );
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(aurora_i18n::tr!("ui-appearance-portrait-current"))
                            .size(12.0)
                            .family(fonts::semibold(ui.ctx()))
                            .color(p.ink),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(aurora_i18n::tr!("ui-appearance-portrait-current-replaced"))
                                .size(12.0)
                                .color(p.muted),
                        )
                        .wrap(),
                    );
                });
            });
        });
}

/// Cancel and Take (Retake and Use once the photo is shown), and the way
/// to the photo mode.
fn footer_ui(ui: &mut egui::Ui, p: &Palette, view: &View, actions: &mut Vec<Action>) {
    let height = 36.0;
    if let Some(cost) = view.hd.confirm.filter(|_| view.shot.is_some()) {
        hd_confirm(ui, p, cost, false, actions);
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if view.shot.is_some() {
            let retake = aurora_i18n::tr!("ui-appearance-portrait-retake");
            let width = widgets::action_button_fit(ui, "refresh", retake);
            if widgets::action_button(ui, p, "refresh", retake, widgets::ActionKind::Flat, Vec2::new(width, height)).clicked() {
                actions.push(Action::Retake);
            }
            let size = Vec2::new(ui.available_width(), height);
            let use_it = aurora_i18n::tr!("ui-appearance-portrait-use");
            if widgets::action_button(ui, p, "check", use_it, widgets::ActionKind::Primary, size).clicked() {
                actions.push(Action::Use);
            }
            return;
        }
        let cancel = aurora_i18n::tr!("common-cancel");
        let width = widgets::action_button_fit(ui, "x", cancel);
        if widgets::action_button(ui, p, "x", cancel, widgets::ActionKind::Flat, Vec2::new(width, height)).clicked() {
            actions.push(Action::Cancel);
        }
        let size = Vec2::new(ui.available_width(), height);
        if view.shooting {
            ui.add_sized(size, |ui: &mut egui::Ui| {
                widgets::busy_button(ui, p, aurora_i18n::tr!("ui-appearance-portrait-taking"))
            });
        } else {
            let take = aurora_i18n::tr!("ui-appearance-portrait-take");
            // (a photo of another outfit than the one it is for is no use)
            let ready = view.worn;
            if widgets::action_button_if(ui, p, "camera-filled", take, widgets::ActionKind::Primary, size, ready).clicked() && ready {
                actions.push(Action::Shoot);
            }
        }
    });
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        if ui
            .add(
                egui::Button::new(
                    RichText::new(aurora_i18n::tr!("ui-appearance-portrait-adjust"))
                        .size(12.0)
                        .color(p.violet_light),
                )
                .frame(false),
            )
            .clicked()
        {
            actions.push(Action::PhotoMode);
        }
    });
}

/// Once the photo is over: saved or not, and the previous outfit back.
fn done(ui: &mut egui::Ui, p: &Palette, view: &View, actions: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing.y = 10.0;
    let (icon, text) = if view.saved {
        ("square-check-filled", aurora_i18n::tr!("ui-appearance-portrait-saved"))
    } else {
        ("info-circle-filled", aurora_i18n::tr!("ui-appearance-portrait-not-saved"))
    };
    note(ui, p, icon, text);
    let previous = view.previous.flatten();
    if previous.is_none() {
        note(
            ui,
            p,
            "info-circle-filled",
            aurora_i18n::tr!("ui-appearance-portrait-previous-unknown"),
        );
    }
    let keep = aurora_i18n::tr!("ui-appearance-portrait-keep-this");
    let mut buttons = vec![widgets::DialogButton {
        icon: "hanger-2-filled",
        label: keep,
        kind: if previous.is_some() {
            widgets::ActionKind::Flat
        } else {
            widgets::ActionKind::Primary
        },
        enabled: true,
    }];
    let put_back = previous.map(|name| aurora_i18n::tr!("ui-appearance-portrait-put-back", outfit = name).to_string());
    if let Some(label) = &put_back {
        buttons.push(widgets::DialogButton {
            icon: "arrow-back-up",
            label,
            kind: widgets::ActionKind::Primary,
            enabled: true,
        });
    }
    match widgets::dialog_buttons(ui, p, &buttons, 32.0) {
        Some(0) => actions.push(Action::Keep),
        Some(_) => actions.push(Action::PutBack),
        None => {}
    }
}

/// The square photo mode over the viewfinder: the banner with the
/// outfit's name and its X, the helpers on the left, how it goes on the
/// right, and once the photo is taken, the photo with Use and Retake.
pub fn photo_mode(ctx: &egui::Context, p: &Palette, settings: &mut PortraitSettings, view: &View) -> Vec<Action> {
    let mut actions = Vec::new();
    let screen = ctx.content_rect();
    let glass = p.navy.gamma_multiply(0.82);
    egui::Area::new(egui::Id::unique("outfit_photo_banner"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(screen.center_top() + Vec2::new(0.0, 60.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(glass)
                .corner_radius(ui.visuals().window_corner_radius)
                .inner_margin(egui::Margin {
                    left: 14,
                    right: 8,
                    top: 6,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        if let Some(t) = icons::global("hanger-2-filled") {
                            ui.add(egui::Image::new(&t).fit_to_exact_size(Vec2::splat(16.0)).tint(p.violet_light));
                        }
                        let mut job = egui::text::LayoutJob::default();
                        let font = egui::FontId::proportional(13.0);
                        job.append(
                            &format!("{} ", aurora_i18n::tr!("ui-appearance-portrait-vf-banner")),
                            0.0,
                            egui::TextFormat::simple(font.clone(), p.ink),
                        );
                        job.append(view.name, 0.0, fonts::semibold_format(&fonts::semibold(ui.ctx()), 13.0, p.ink));
                        job.append(
                            &format!("  · {}", aurora_i18n::tr!("ui-appearance-portrait-vf-square")),
                            0.0,
                            egui::TextFormat::simple(font, p.muted),
                        );
                        ui.label(job);
                        let exit = aurora_i18n::tr!("ui-appearance-portrait-vf-exit");
                        if widgets::icon_button(ui, p, "x", exit, true) {
                            actions.push(Action::Cancel);
                        }
                    });
                });
        });
    // the helpers, low on the left: the viewfinder's key list holds the
    // top left corner and its focus modes the bottom
    egui::Area::new(egui::Id::unique("outfit_photo_helpers"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(screen.left_bottom() + Vec2::new(24.0, -80.0))
        .show(ctx, |ui| {
            ui.set_width(230.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new(aurora_i18n::tr!("ui-appearance-portrait-vf-helpers").to_uppercase())
                    .size(11.0)
                    .family(fonts::semibold(ui.ctx()))
                    .color(p.muted),
            );
            for (icon, label, on, action) in [
                (
                    "focus-centered",
                    aurora_i18n::tr!("ui-appearance-portrait-vf-reframe"),
                    false,
                    Action::Reframe,
                ),
                (
                    "man-filled",
                    aurora_i18n::tr!("ui-appearance-portrait-vf-pose"),
                    view.pose,
                    Action::Pose,
                ),
                (
                    "user-scan",
                    aurora_i18n::tr!("ui-appearance-portrait-vf-studio"),
                    view.studio,
                    Action::Studio,
                ),
            ] {
                if helper(ui, p, icon, label, on).clicked() {
                    actions.push(action);
                }
            }
            if !view.worn {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(glass)
                    .corner_radius(ui.visuals().window_corner_radius)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let text = if view.previous.is_some() {
                            aurora_i18n::tr!("ui-appearance-portrait-wearing")
                        } else {
                            aurora_i18n::tr!("ui-appearance-portrait-not-worn")
                        };
                        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(p.ink)).wrap());
                        if view.previous.is_none() {
                            if view.dirty {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(aurora_i18n::tr!("ui-appearance-portrait-unsaved"))
                                            .size(12.0)
                                            .color(p.muted),
                                    )
                                    .wrap(),
                                );
                            }
                            let label = aurora_i18n::tr!("ui-appearance-portrait-wear");
                            let size = Vec2::new(ui.available_width(), 30.0);
                            if widgets::action_button(ui, p, "hanger-2-filled", label, widgets::ActionKind::Primary, size).clicked() {
                                actions.push(Action::Wear);
                            }
                        }
                    });
            }
        });
    // how it goes, low on the right, clear of the viewfinder's button
    // rail and its depth of field readout
    egui::Area::new(egui::Id::unique("outfit_photo_how"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(screen.right_bottom() + Vec2::new(-100.0, -120.0))
        .show(ctx, |ui| {
            ui.set_width(230.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new(aurora_i18n::tr!("ui-appearance-portrait-vf-how").to_uppercase())
                    .size(11.0)
                    .family(fonts::semibold(ui.ctx()))
                    .color(p.muted),
            );
            egui::Frame::new()
                .fill(glass)
                .corner_radius(ui.visuals().window_corner_radius)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(
                        egui::Label::new(
                            RichText::new(aurora_i18n::tr!("ui-appearance-portrait-vf-how-text"))
                                .size(12.0)
                                .color(p.muted),
                        )
                        .wrap(),
                    );
                });
        });
    // the photo taken, to confirm
    if let Some(t) = view.shot.filter(|_| !view.shooting) {
        egui::Area::new(egui::Id::unique("outfit_photo_shot"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .pivot(Align2::CENTER_CENTER)
            .fixed_pos(screen.center())
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.panel)
                    .stroke(egui::Stroke::new(1.0, p.raised))
                    .corner_radius(ui.visuals().window_corner_radius)
                    .inner_margin(14.0)
                    .show(ui, |ui| {
                        let side = (screen.height() * 0.45).clamp(160.0, 320.0);
                        ui.set_width(side);
                        ui.spacing_mut().item_spacing.y = 10.0;
                        ui.add(
                            egui::Image::new(t)
                                .fit_to_exact_size(Vec2::splat(side))
                                .corner_radius(ui.visuals().window_corner_radius),
                        );
                        if let Some(cost) = view.hd.confirm {
                            hd_confirm(ui, p, cost, true, &mut actions);
                            return;
                        }
                        hd_choice(ui, p, settings, &view.hd);
                        let height = 34.0;
                        let use_it = aurora_i18n::tr!("ui-appearance-portrait-vf-use");
                        if widgets::action_button(ui, p, "check", use_it, widgets::ActionKind::Primary, Vec2::new(side, height)).clicked() {
                            actions.push(Action::Use);
                        }
                        let retake = aurora_i18n::tr!("ui-appearance-portrait-retake");
                        if widgets::action_button(ui, p, "refresh", retake, widgets::ActionKind::Flat, Vec2::new(side, height)).clicked() {
                            actions.push(Action::Retake);
                        }
                    });
            });
    }
    actions
}

/// A helper of the photo mode: an icon and its label on glass, outlined
/// in the accent while on.
fn helper(ui: &mut egui::Ui, p: &Palette, icon: &str, label: &str, on: bool) -> egui::Response {
    let size = Vec2::new(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, label));
    let face = if response.hovered() {
        p.raised
    } else {
        p.navy.gamma_multiply(0.82)
    };
    let radius = ui.visuals().window_corner_radius;
    ui.painter().rect_filled(rect, radius, face);
    if on {
        ui.painter()
            .rect_stroke(rect, radius, egui::Stroke::new(1.0, p.violet_light), egui::StrokeKind::Inside);
    }
    if let Some(t) = icons::global(icon) {
        let at = Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), Vec2::splat(16.0));
        ui.painter()
            .image(t.id(), at, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), p.ink);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 36.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        p.ink,
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View<'static> {
        View {
            name: "Velvet evening",
            preview: None,
            shot: None,
            current: None,
            worn: false,
            previous: None,
            dirty: true,
            shooting: false,
            over: false,
            saved: false,
            pose: false,
            studio: false,
            hd: HdView {
                offer: HdOffer::Short(10),
                busy: false,
                confirm: None,
                has_one: true,
            },
        }
    }

    #[test]
    fn the_window_keeps_its_size() {
        crate::ui::widgets::steady_tests::assert_window_steady(
            WINDOW,
            &[egui::vec2(780.0, 600.0), egui::vec2(560.0, 480.0)],
            aurora_i18n::tr!("ui-appearance-portrait-adjust"),
            PortraitSettings::default,
            |settings, ctx, p| {
                window(ctx, p, settings, &view(), &mut true);
            },
        );
    }

    #[test]
    fn the_portrait_frame_fits_the_window_height_without_changing_its_ratio() {
        let available = Vec2::new(640.0, 420.0);
        let portrait = frame_size(available, Ratio::Portrait.aspect());
        assert!(portrait.x <= available.x && portrait.y <= available.y);
        assert!((portrait.x / portrait.y - Ratio::Portrait.aspect()).abs() < 1e-5);

        let short_window = Vec2::new(640.0, 220.0);
        let portrait = frame_size(short_window, Ratio::Portrait.aspect());
        assert!(portrait.y <= short_window.y);
        assert!((portrait.x / portrait.y - Ratio::Portrait.aspect()).abs() < 1e-5);
    }
}
