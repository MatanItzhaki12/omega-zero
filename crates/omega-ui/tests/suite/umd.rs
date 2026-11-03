//! The PSP's UMD on the shelf: a smoky clear shell with its disc showing through it round the
//! label, the disc catching the light as the label's holo sheen does.

use omega_store::{Cart, Platform, ShellChoice};
use omega_ui::{
    cart_face, holo_frames, model_box, model_of, shell_for, CartModel, Finish, HOLO_FRAMES, UMD_H,
    UMD_SHELL, UMD_W,
};

fn game(platform: Platform, shell: &str) -> Cart {
    Cart {
        platform,
        stem: "Monster Hunter Freedom Unite (USA)".into(),
        rom: "none.iso".into(),
        label: None,
        title: "Monster Hunter Freedom Unite".into(),
        code: String::new(),
        shell: ShellChoice::parse(shell),
    }
}

fn alpha(face: &omega_ui::CartFace, x: u32, y: u32) -> u8 {
    face.rgba[((y * face.w + x) * 4 + 3) as usize]
}

/// On the disc's silver face, under the label.
const ON_DISC: (u32, u32) = (138, 234);
/// In the shell's top corner, clear of the disc and the label.
const HOLLOW: (u32, u32) = (60, 16);
/// Under the label, where the disc's clear ring is.
const OVER_RING: (u32, u32) = (138, 94);

#[test]
fn a_psp_game_stands_as_a_umd_and_a_playstation_game_as_a_disc() {
    assert_eq!(model_of(&game(Platform::Psp, "")), CartModel::Umd);
    assert_eq!(model_of(&game(Platform::Ps1, "")), CartModel::Disc);
    assert_eq!(model_box(CartModel::Umd), (UMD_W, UMD_H));
}

#[test]
fn a_umd_comes_in_smoky_clear_plastic_unless_set_otherwise() {
    let shell = shell_for(&game(Platform::Psp, ""));
    assert_eq!(shell, UMD_SHELL);
    assert_eq!(shell.finish, Finish::Translucent);
    let red = shell_for(&game(Platform::Psp, "auto c2332e clear"));
    assert_eq!(red.colour, [0xc2, 0x33, 0x2e]);
}

#[test]
fn the_disc_shows_through_the_shell_and_the_label_hides_it() {
    let face = cart_face(&game(Platform::Psp, ""));
    assert_eq!((face.w, face.h), (UMD_W, UMD_H));
    assert_eq!(
        alpha(&face, ON_DISC.0, ON_DISC.1),
        255,
        "the disc lets the shelf through"
    );
    let hollow = alpha(&face, HOLLOW.0, HOLLOW.1);
    assert!(hollow < 255, "the empty shell is not see-through: {hollow}");
    assert!(hollow > 0, "the shell is not there at all");
    assert_eq!(
        alpha(&face, OVER_RING.0, OVER_RING.1),
        255,
        "the disc's clear ring shows through the label"
    );
    // outside the round top corner there is no shell
    assert_eq!(alpha(&face, 2, 2), 0);
}

#[test]
fn the_disc_throws_the_light_as_the_umd_turns_and_the_empty_shell_does_not() {
    let frames = holo_frames(CartModel::Umd);
    assert_eq!(frames.len(), HOLO_FRAMES);
    let half = |(x, y): (u32, u32)| (x / 2, y / 2);
    let (dx, dy) = half(ON_DISC);
    let lit: Vec<u8> = frames.iter().map(|f| alpha(f, dx, dy)).collect();
    assert!(
        lit.iter().any(|&a| a > 60),
        "the disc never catches the light: {lit:?}"
    );
    assert!(
        lit.iter().any(|&a| a < 10),
        "the disc's light does not move off it: {lit:?}"
    );
    let (hx, hy) = half(HOLLOW);
    assert!(
        frames.iter().all(|f| alpha(f, hx, hy) == 0),
        "the empty shell shines"
    );
}
