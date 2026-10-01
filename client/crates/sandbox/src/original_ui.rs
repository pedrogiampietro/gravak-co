//! Component catalog and composition helpers for the original 5065 UI.

use crate::assets::{Assets, Result};
use image::{Rgba, RgbaImage, imageops};

pub const LOGICAL_WIDTH: u32 = 1024;
pub const LOGICAL_HEIGHT: u32 = 768;
pub const MAIN_HUD_RECT: [u32; 4] = [0, 627, 1024, 141];
pub const CHARACTER_RECT: [u32; 4] = [0, 65, 262, 406];
pub const NPC_SHOP_RECT: [u32; 4] = [0, 65, 272, 250];
pub const INVENTORY_RECT: [u32; 4] = [754, 0, 270, 502];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterComponent {
    Shell,
    Role,
    RoleMore,
    Equipment,
    Weapon,
    Mission,
}

impl CharacterComponent {
    pub const fn asset(self) -> &'static str {
        match self {
            Self::Shell => "data/main/DialogQuery.dds",
            Self::Role => "data/main/DialogQuery_Role.dds",
            Self::RoleMore => "data/main/DialogQuery_Role_More.dds",
            Self::Equipment => "data/main/DialogNpcEquip.dds",
            Self::Weapon => "data/main/DialogQuery_Weapon.dds",
            Self::Mission => "data/main/DialogQuery_Mission.dds",
        }
    }
}

pub const INVENTORY_ASSET: &str = "data/main/DialogMpcGoods.dds";
pub const WAREHOUSE_ASSET: &str = "data/main/DialogDepot.dds";
pub const CLOSE_ASSET: &str = "data/main/CloseBtnS.dds";
pub const CLOSE_HOVER_ASSET: &str = "data/main/CloseBtnSclick.dds";
pub const ROLE_BASE_BUTTON_ASSET: &str = "data/main/RoleBtnBase.dds";
pub const ROLE_MORE_BUTTON_ASSET: &str = "data/main/RoleBtnMore.dds";

/// Draws an original control texture with its canvas origin at `(x, y)`.
/// `GUI.ini` rectangles address the texture canvas, transparent margin
/// included, so the image is not cropped first.
pub fn overlay_asset(
    target: &mut RgbaImage,
    assets: &mut Assets,
    path: &str,
    x: i64,
    y: i64,
) -> Result<()> {
    let source = assets.image(path)?;
    imageops::overlay(target, &source, x, y);
    Ok(())
}

pub fn component(assets: &mut Assets, component: CharacterComponent) -> Result<RgbaImage> {
    let source = assets.image(component.asset())?;
    let [x, y, width, height] = component.native_rect();
    Ok(imageops::crop_imm(&source, x, y, width, height).to_image())
}

pub fn inventory_base(assets: &mut Assets) -> Result<RgbaImage> {
    let source = assets.image(INVENTORY_ASSET)?;
    let mut base =
        imageops::crop_imm(&source, 0, 0, INVENTORY_RECT[2], INVENTORY_RECT[3]).to_image();
    // GUI.ini [153-1561] money button, [153-1667] Help, [153-1128] Close.
    overlay_asset(&mut base, assets, "data/main/MpcGoodsMoneyBtn.dds", 71, 390)?;
    // `[Dialog57]` CPs (晶石) icon beside the second field ([153-1562]).
    overlay_asset(&mut base, assets, "data/main/PublicBtnMoney.dds", 71, 422)?;
    overlay_asset(&mut base, assets, "data/main/HelpBtn2.dds", 45, 447)?;
    overlay_asset(&mut base, assets, "data/main/CloseBtn2.dds", 170, 447)?;
    Ok(base)
}

pub fn close_button(assets: &mut Assets, hovered: bool) -> Result<RgbaImage> {
    let path = if hovered {
        CLOSE_HOVER_ASSET
    } else {
        CLOSE_ASSET
    };
    visible_part(&assets.image(path)?)
}

pub fn role_button(assets: &mut Assets, more: bool) -> Result<RgbaImage> {
    let path = if more {
        ROLE_MORE_BUTTON_ASSET
    } else {
        ROLE_BASE_BUTTON_ASSET
    };
    visible_part(&assets.image(path)?)
}

impl CharacterComponent {
    /// Native canvas occupied by each component in the 5065 UI atlas.
    ///
    /// These rectangles deliberately retain transparent margins. The GUI
    /// coordinates in `GUI.ini` are relative to the atlas canvas, not to the
    /// first non-transparent pixel. Cropping by alpha here shifts equipment
    /// slots and clips the bottom of `DialogNpcEquip`.
    pub const fn native_rect(self) -> [u32; 4] {
        match self {
            Self::Shell => [0, 0, 264, 435],
            Self::Role | Self::Weapon | Self::Mission => [0, 0, 256, 322],
            Self::RoleMore => [0, 0, 256, 128],
            Self::Equipment => [0, 0, 256, 440],
        }
    }
}

/// Removes only the transparent canvas surrounding an original DDS asset.
pub fn visible_part(source: &RgbaImage) -> Result<RgbaImage> {
    let mut min_x = source.width();
    let mut min_y = source.height();
    let mut max_x = None;
    let mut max_y = None;
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = Some(max_x.map_or(x, |value: u32| value.max(x)));
        max_y = Some(max_y.map_or(y, |value: u32| value.max(y)));
    }
    let (Some(max_x), Some(max_y)) = (max_x, max_y) else {
        return Ok(RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0])));
    };
    Ok(imageops::crop_imm(source, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_components_keep_the_5065_asset_contract() {
        assert_eq!(
            CharacterComponent::Shell.asset(),
            "data/main/DialogQuery.dds"
        );
        assert_eq!(
            CharacterComponent::Role.asset(),
            "data/main/DialogQuery_Role.dds"
        );
        assert_eq!(
            CharacterComponent::Equipment.asset(),
            "data/main/DialogNpcEquip.dds"
        );
        assert_eq!(INVENTORY_ASSET, "data/main/DialogMpcGoods.dds");
        assert_eq!(MAIN_HUD_RECT, [0, 627, 1024, 141]);
        assert_eq!(
            CharacterComponent::Equipment.native_rect(),
            [0, 0, 256, 440]
        );
        assert_eq!(INVENTORY_RECT, [754, 0, 270, 502]);
    }

    #[test]
    fn visible_part_drops_only_transparent_margin() {
        let mut image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        image.put_pixel(2, 3, Rgba([255, 255, 255, 255]));
        image.put_pixel(4, 5, Rgba([255, 255, 255, 255]));
        let cropped = visible_part(&image).expect("crop");
        assert_eq!(cropped.dimensions(), (3, 3));
        assert_eq!(cropped.get_pixel(0, 0)[3], 255);
        assert_eq!(cropped.get_pixel(2, 2)[3], 255);
    }
}
