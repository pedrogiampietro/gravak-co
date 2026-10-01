use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};
use tq_serde::StringList;

use crate::constants;

#[derive(Debug, Clone, Copy, PartialEq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum DialogActionKind {
    #[default]
    Unknown = 0,
    Text = 1,
    Link = 2,
    Edit = 3,
    Avatar = 4,
    ListLine = 5,
    Create = 100,
    Answer = 101,
    TaskId = 102,
}

/// This packet is used to interact with a NPC and contains multiple
/// DialogAction types that are used to determine the type of interaction.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2032)]
pub struct MsgTaskDialog {
    task_id: u32,
    avatar: u16,
    option_id: u8,
    action: u8,
    msgs: StringList,
}

impl MsgTaskDialog {
    pub fn builder() -> MultiTaskDialogBuilder<AddingText> {
        MultiTaskDialogBuilder::default()
    }
}

/// A Multi Task Dialog builder helps crafting a dialog with multiple options
/// and actions by building a list of [`MsgTaskDialog`] packets in very
/// chainable and ergonomic way.
#[derive(Debug, Clone, Default)]
pub struct MultiTaskDialogBuilder<S> {
    /// Holds the list of tasks that will be sent to the client.
    tasks: Vec<MsgTaskDialog>,
    /// Holds the current state.
    _state: std::marker::PhantomData<S>,
}

// Tasks are sent in a specific order, so we have different states to
// ensure that the builder is used correctly.
#[derive(Debug, Clone, Default)]
pub struct AddingText;
#[derive(Debug, Clone)]
pub struct AddingOptionOrEdit;
#[derive(Debug, Clone)]
pub struct AddingAvatar;
#[derive(Debug, Clone)]
pub struct Ready;

impl MultiTaskDialogBuilder<Ready> {
    pub fn task_id(mut self, task_id: u32) -> Self {
        for task in &mut self.tasks {
            task.task_id = task_id;
        }
        self
    }

    /// Builds the list of tasks and returns it.
    pub fn build(mut self) -> Vec<MsgTaskDialog> {
        // Push the last task
        self.tasks.push(MsgTaskDialog {
            task_id: 0,
            avatar: 0,
            option_id: u8::MAX,
            action: DialogActionKind::Create.into(),
            msgs: Default::default(),
        });
        self.tasks
    }
}

impl MultiTaskDialogBuilder<AddingText> {
    /// Adds a text task to the list of tasks.
    ///
    /// # Arguments
    ///
    /// * `text` - A string slice representing the text to be added to the task.
    pub fn text<T: AsRef<str>>(mut self, text: T) -> MultiTaskDialogBuilder<AddingOptionOrEdit> {
        let text = text.as_ref();
        let msgs = text
            .as_bytes()
            .chunks(constants::MAX_TXT_LEN)
            .flat_map(std::str::from_utf8)
            .collect();
        self.tasks.push(MsgTaskDialog {
            task_id: 0,
            avatar: 0,
            option_id: u8::MAX,
            action: DialogActionKind::Text.into(),
            msgs,
        });
        MultiTaskDialogBuilder {
            tasks: self.tasks,
            _state: std::marker::PhantomData,
        }
    }
}

impl MultiTaskDialogBuilder<AddingOptionOrEdit> {
    fn mk_option<T: AsRef<str>>(mut self, option_id: u8, text: T, action: DialogActionKind) -> Self {
        let s = text.as_ref();
        // Truncate the text to the maximum allowed length.
        let option_text = if s.len() <= constants::MAX_TXT_LEN {
            s
        } else {
            // Take the first MAX_TXT_LEN characters.
            match s.char_indices().nth(constants::MAX_TXT_LEN) {
                None => s,
                Some((idx, _)) => &s[..idx],
            }
        };
        self.tasks.push(MsgTaskDialog {
            task_id: 0,
            avatar: 0,
            option_id,
            action: action.into(),
            msgs: vec![option_text].into(),
        });
        self
    }

    /// Adds an option task to the list of tasks.
    ///
    /// # Arguments
    ///
    /// * `option_id` - A u8 representing the option id to be added to the task.
    /// * `text` - A string slice representing the text to be added to the task.
    pub fn with_option<T: AsRef<str>>(self, option_id: u8, text: T) -> Self {
        self.mk_option(option_id, text, DialogActionKind::Link)
    }

    /// Adds an edit task to the list of tasks.
    ///
    /// # Arguments
    ///
    /// * `option_id` - A u8 representing the option id to be added to the task.
    /// * `text` - A string slice representing the text to be added to the task.
    pub fn with_edit<T: AsRef<str>>(self, option_id: u8, text: T) -> Self {
        self.mk_option(option_id, text, DialogActionKind::Edit)
    }

    /// Transitions to the next state.
    pub fn and(self) -> MultiTaskDialogBuilder<AddingAvatar> {
        MultiTaskDialogBuilder {
            tasks: self.tasks,
            _state: std::marker::PhantomData,
        }
    }
}

impl MultiTaskDialogBuilder<AddingAvatar> {
    /// Adds an avatar task to the list of tasks.
    ///
    /// # Arguments
    ///
    /// * `avatar` - A u16 representing the avatar to be added to the task.
    pub fn with_avatar(mut self, avatar: u16) -> MultiTaskDialogBuilder<Ready> {
        self.tasks.push(MsgTaskDialog {
            task_id: 0,
            avatar,
            option_id: u8::MAX,
            action: DialogActionKind::Avatar.into(),
            msgs: Default::default(),
        });
        MultiTaskDialogBuilder {
            tasks: self.tasks,
            _state: std::marker::PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl PacketProcess for MsgTaskDialog {
    type ActorState = crate::ActorState;
    type Error = crate::Error;
    type State = crate::State;

    async fn process(&self, _state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        tracing::debug!(msg = ?self, "MsgTaskDialog received");
        if self.action != DialogActionKind::Answer as u8 {
            return Ok(());
        }
        if self.option_id == u8::MAX {
            return Ok(());
        }
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let npc = _state
            .try_map(character.entity().map_id())
            .ok()
            .and_then(|map| map.npc(self.task_id));
        if let Some(npc) = npc.as_ref() {
            if let Some(teleport) = crate::teleports::resolve(self.task_id, self.option_id) {
                let player = character.entity().location();
                let location = npc.entity().location();
                if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "The city service NPC is too far away.",
                        ))
                        .await?;
                    return Ok(());
                }
                return city_teleport(_state, actor, character, npc.id(), teleport).await;
            }
        }
        if matches!(self.option_id, 1 | 2 | 3)
            && npc.is_some_and(|npc| npc.kind() == crate::entities::NpcKind::ShopKeeper)
        {
            character.set_active_shop_id(self.task_id);
        }
        if self.option_id == 1 && npc.is_some_and(|npc| npc.kind() == crate::entities::NpcKind::Storage) {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "Warehouse is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let storage_id = npc.id();
            character.set_active_storage_id(storage_id);
            character.load_warehouse(_state.pool()).await?;
            actor.send(super::MsgItemInfo::clear_warehouse(storage_id)).await?;
            actor
                .send(super::MsgItemInfo::warehouse_balance(
                    storage_id,
                    character.warehouse_silver(),
                ))
                .await?;
            for item in character.warehouse_items() {
                actor.send(super::MsgItemInfo::warehouse_item(item)).await?;
            }
            return Ok(());
        }
        if self.task_id == 425 && self.option_id == 4 {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "Old General Yang is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let response = match crate::quests::start(_state.pool(), character.database_id(), 1).await? {
                crate::quests::StartOutcome::Started(quest) => {
                    format!("Quest accepted: {}. {}", quest.name, quest.description)
                },
                crate::quests::StartOutcome::AlreadyActive(quest, progress) => format!(
                    "Quest already active: {} ({}/{}).",
                    quest.name, progress, quest.required_count
                ),
                crate::quests::StartOutcome::AlreadyCompleted(quest) => {
                    format!("Quest already completed: {}.", quest.name)
                },
                crate::quests::StartOutcome::NotFound => "This quest is not available.".to_owned(),
            };
            actor
                .send(super::MsgTalk::from_system(
                    character.id(),
                    super::TalkChannel::System,
                    response,
                ))
                .await?;
            return Ok(());
        }
        let character_name = entity.basic().name().to_owned();
        let npc_name = npc.map(|value| value.entity().name().to_owned());
        let npc_name_lower = npc_name.as_deref().unwrap_or_default().to_ascii_lowercase();
        let is_training_npc = npc_name_lower.contains("trainer")
            || npc_name_lower.contains("star")
            || npc_name_lower.contains("god")
            || npc_name_lower.contains("general");
        let is_upgrade_npc = npc.is_some_and(|npc| npc.kind() == crate::entities::NpcKind::Forge)
            || npc_name_lower.contains("artisan")
            || npc_name_lower.contains("master");
        let is_jeweler = npc_name_lower.contains("jewel");
        if is_jeweler && matches!(self.option_id, 1 | 2) {
            let Some(npc) = npc else { return Ok(()) };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The jeweler is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let tier = self.option_id;
            let mut dialog = super::MsgTaskDialog::builder().text(format!(
                "Choose a gem family. Fifteen equal gems cost {} silver.",
                if tier == 1 { 10_000 } else { 100_000 }
            ));
            let inventory = character.inventory_items();
            let mut available = false;
            for (index, base) in crate::entities::GEM_FAMILY_BASES.iter().enumerate() {
                let item_type = *base + u32::from(tier);
                let count = inventory.iter().filter(|item| item.item_type == item_type).count();
                if count >= 15 {
                    available = true;
                    dialog = dialog.with_option(
                        if tier == 1 { 20 + index as u8 } else { 30 + index as u8 },
                        format!("Compose gem {} ({} available).", item_type, count),
                    );
                }
            }
            if !available {
                dialog = dialog.with_option(255, "I do not have fifteen equal gems.");
            }
            for message in dialog.and().with_avatar(47).task_id(npc.id()).build() {
                actor.send(message).await?;
            }
            return Ok(());
        }
        let compose_tier = if (20..28).contains(&self.option_id) {
            Some(1_u32)
        } else if (30..38).contains(&self.option_id) {
            Some(2_u32)
        } else {
            None
        };
        if is_jeweler && compose_tier.is_some() {
            let Some(npc) = npc else { return Ok(()) };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The jeweler is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let tier = compose_tier.unwrap_or_default();
            let index = if tier == 1 {
                self.option_id - 20
            } else {
                self.option_id - 30
            };
            let Some(base) = crate::entities::GEM_FAMILY_BASES.get(usize::from(index)) else {
                return Ok(());
            };
            let gem_type = *base + tier;
            match character.compose_gems(_state.pool(), gem_type).await? {
                crate::entities::GemComposeOutcome::Composed {
                    consumed,
                    created,
                    silver,
                } => {
                    crate::audit::economic("gem_compose", character.id(), npc.id(), created.uid, silver);
                    crate::audit::item(
                        "gem_compose_create",
                        character.id(),
                        created.uid,
                        created.item_type,
                        format_args!("silver={} consumed={}", silver, consumed.len()),
                    );
                    for item in consumed {
                        actor.send(super::MsgItemInfo::remove_inventory_item(item)).await?;
                    }
                    actor.send(super::MsgItemInfo::inventory_item(created)).await?;
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::System,
                            format!(
                                "Composed gem {} into {} for {} silver.",
                                gem_type, created.item_type, silver
                            ),
                        ))
                        .await?;
                },
                crate::entities::GemComposeOutcome::MissingGems => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "You need fifteen equal gems.",
                        ))
                        .await?
                },
                crate::entities::GemComposeOutcome::InsufficientSilver => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "You do not have enough silver.",
                        ))
                        .await?
                },
                _ => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "That gem cannot be composed here.",
                        ))
                        .await?
                },
            }
            return Ok(());
        }
        if is_upgrade_npc && self.option_id == 1 {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The artisan is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let mut dialog = super::MsgTaskDialog::builder()
                .text("Choose an equipped item to improve. Each upgrade consumes one Meteor.");
            let equipped = character.equipment_items();
            if equipped.is_empty() {
                dialog = dialog.with_option(255, "I have no equipment ready.");
            } else {
                for item in equipped {
                    dialog = dialog.with_option(
                        10_u8.saturating_add(item.position),
                        format!(
                            "Improve slot {} (item {} +{}).",
                            item.position, item.item_type, item.plus
                        ),
                    );
                }
                dialog = dialog.with_option(255, "Leave.");
            }
            let dialog = dialog.and().with_avatar(47).task_id(npc.id()).build();
            for message in dialog {
                actor.send(message).await?;
            }
            return Ok(());
        }
        if is_upgrade_npc && self.option_id == 2 {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The artisan is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let mut dialog = super::MsgTaskDialog::builder()
                .text("Choose an equipped item to refine. One DragonBall is consumed per attempt.");
            let equipped = character.equipment_items();
            if equipped.is_empty() {
                dialog = dialog.with_option(255, "I have no equipment ready.");
            } else {
                for item in equipped {
                    let chance = crate::entities::Character::quality_upgrade_chance(item.item_type);
                    dialog = dialog.with_option(
                        20_u8.saturating_add(item.position),
                        format!(
                            "Refine slot {} (item {}, {}% chance).",
                            item.position, item.item_type, chance
                        ),
                    );
                }
                dialog = dialog.with_option(255, "Leave.");
            }
            for message in dialog.and().with_avatar(47).task_id(npc.id()).build() {
                actor.send(message).await?;
            }
            return Ok(());
        }
        if is_upgrade_npc && (10..20).contains(&self.option_id) {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The artisan is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let position = self.option_id - 10;
            let Some(target) = character
                .equipment_items()
                .into_iter()
                .find(|item| item.position == position)
            else {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        "There is no equipment in that slot.",
                    ))
                    .await?;
                return Ok(());
            };
            let (outcome, catalysts) = character.upgrade_item_with_meteor(_state.pool(), target.uid).await?;
            match outcome {
                crate::entities::ItemUpgradeOutcome::Upgraded(item) => {
                    for catalyst in catalysts {
                        actor.send(super::MsgItemInfo::remove_inventory_item(catalyst)).await?;
                    }
                    actor.send(super::MsgItemInfo::equipment_item(item)).await?;
                    character.refresh_equipment_resources(_state.shop_catalog());
                    actor.send(super::MsgUserInfo::from(character)).await?;
                    actor.send(super::MsgPlayer::from(character)).await?;
                    actor.screen().send_message(super::MsgPlayer::from(character)).await?;
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::System,
                            format!("Item {} upgraded to +{}.", item.item_type, item.plus),
                        ))
                        .await?;
                },
                crate::entities::ItemUpgradeOutcome::Failed(item) => {
                    for catalyst in catalysts {
                        actor.send(super::MsgItemInfo::remove_inventory_item(catalyst)).await?;
                    }
                    actor.send(super::MsgItemInfo::equipment_item(item)).await?;
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "Upgrade failed; the Meteors were consumed.",
                        ))
                        .await?;
                },
                crate::entities::ItemUpgradeOutcome::MaxLevel(item) => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            format!("Item {} is already at maximum +{}.", item.item_type, item.plus),
                        ))
                        .await?;
                },
                crate::entities::ItemUpgradeOutcome::MissingMeteor => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "Upgrade rejected: you need a Meteor.",
                        ))
                        .await?;
                },
                crate::entities::ItemUpgradeOutcome::NotFound => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "Upgrade rejected: the item is no longer equipped.",
                        ))
                        .await?;
                },
            }
            return Ok(());
        }
        if is_upgrade_npc && (20..30).contains(&self.option_id) {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The artisan is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let position = self.option_id - 20;
            let Some(target) = character
                .equipment_items()
                .into_iter()
                .find(|item| item.position == position)
            else {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        "There is no equipment in that slot.",
                    ))
                    .await?;
                return Ok(());
            };
            let (outcome, catalyst) = character
                .upgrade_quality_with_dragonball(_state.pool(), target.uid)
                .await?;
            match outcome {
                crate::entities::QualityUpgradeOutcome::Upgraded(item)
                | crate::entities::QualityUpgradeOutcome::Failed(item) => {
                    if let Some(catalyst) = catalyst {
                        actor.send(super::MsgItemInfo::remove_inventory_item(catalyst)).await?;
                    }
                    actor.send(super::MsgItemInfo::equipment_item(item)).await?;
                    character.refresh_equipment_resources(_state.shop_catalog());
                    actor.send(super::MsgUserInfo::from(character)).await?;
                    actor.send(super::MsgPlayer::from(character)).await?;
                    actor.screen().send_message(super::MsgPlayer::from(character)).await?;
                    if matches!(outcome, crate::entities::QualityUpgradeOutcome::Upgraded(_)) {
                        actor
                            .send(super::MsgTalk::from_system(
                                character.id(),
                                super::TalkChannel::System,
                                format!("Item {} quality upgraded successfully.", item.item_type),
                            ))
                            .await?;
                    } else {
                        actor
                            .send(super::MsgTalk::from_system(
                                character.id(),
                                super::TalkChannel::TopLeft,
                                format!(
                                    "Quality upgrade failed; durability is now {}/{}.",
                                    item.durability, item.max_durability
                                ),
                            ))
                            .await?;
                    }
                },
                crate::entities::QualityUpgradeOutcome::MaxQuality(item) => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            format!("Item {} is already at maximum quality.", item.item_type),
                        ))
                        .await?
                },
                crate::entities::QualityUpgradeOutcome::InvalidQuality(item) => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            format!("Item {} cannot be refined through quality upgrade.", item.item_type),
                        ))
                        .await?
                },
                crate::entities::QualityUpgradeOutcome::MissingDragonBall => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "Quality upgrade rejected: you need a DragonBall.",
                        ))
                        .await?
                },
                crate::entities::QualityUpgradeOutcome::NotFound => {
                    actor
                        .send(super::MsgTalk::from_system(
                            character.id(),
                            super::TalkChannel::TopLeft,
                            "Quality upgrade rejected: the item is no longer equipped.",
                        ))
                        .await?
                },
            }
            return Ok(());
        }
        if self.option_id == 3 && is_training_npc {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The trainer is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let level = character.entity().level();
            let learned = character.skills();
            let Some(skill) = _state
                .skill_catalog()
                .next_trainer_skill(level, |skill_id| learned.iter().any(|known| known.skill_id == skill_id))
                .cloned()
            else {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        "This trainer has no skill available yet.",
                    ))
                    .await?;
                return Ok(());
            };
            if character.entity().level() < skill.required_level {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        format!("You need to be level {} to learn {}.", skill.required_level, skill.name),
                    ))
                    .await?;
                return Ok(());
            }
            if character.has_skill(skill.id) {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        format!("You already learned {}.", skill.name),
                    ))
                    .await?;
                return Ok(());
            }
            if character.silver() < u64::from(skill.training_cost) {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        format!("Learning {} costs {} silver.", skill.name, skill.training_cost),
                    ))
                    .await?;
                return Ok(());
            }
            let Some(progress) = character
                .learn_skill_with_cost(_state.pool(), skill.id, 1, skill.training_cost)
                .await?
            else {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The training could not be completed; no silver was charged.",
                    ))
                    .await?;
                return Ok(());
            };
            crate::audit::economic("skill_training", character.id(), npc.id(), 0, skill.training_cost);
            actor.send(super::MsgSkill::from(progress)).await?;
            actor.send(super::MsgUserInfo::from(character)).await?;
            actor
                .send(super::MsgTalk::from_system(
                    character.id(),
                    super::TalkChannel::System,
                    format!(
                        "The trainer taught you {} for {} silver.",
                        skill.name, skill.training_cost
                    ),
                ))
                .await?;
            return Ok(());
        }
        if self.option_id == 3
            && npc.is_some_and(|npc| npc.kind() == crate::entities::NpcKind::ShopKeeper)
            && (npc_name_lower.contains("blacksmith")
                || npc_name_lower.contains("armorer")
                || npc_name_lower.contains("weapon"))
        {
            let Some(npc) = npc else {
                return Ok(());
            };
            let player = character.entity().location();
            let location = npc.entity().location();
            if player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        "The blacksmith is too far away.",
                    ))
                    .await?;
                return Ok(());
            }
            let cost = character
                .equipment_items()
                .into_iter()
                .filter_map(|item| {
                    _state
                        .shop_catalog()
                        .repair_price(item.item_type, item.durability, item.max_durability)
                })
                .map(u64::from)
                .sum::<u64>()
                .min(u64::from(u32::MAX)) as u32;
            if cost == 0 {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::System,
                        "All equipped items are already repaired.",
                    ))
                    .await?;
                return Ok(());
            }
            let Some(repaired) = character.repair_all_equipment(_state.pool(), cost).await? else {
                actor
                    .send(super::MsgTalk::from_system(
                        character.id(),
                        super::TalkChannel::TopLeft,
                        format!("Repairing everything costs {cost} silver, but you cannot pay it."),
                    ))
                    .await?;
                return Ok(());
            };
            crate::audit::economic("npc_repair", character.id(), npc.id(), 0, cost);
            for item in repaired {
                crate::audit::item(
                    "repair",
                    character.id(),
                    item.uid,
                    item.item_type,
                    format_args!("npc_id={} cost={}", npc.id(), cost),
                );
                actor.send(super::MsgItemInfo::equipment_item(item)).await?;
            }
            actor.send(super::MsgUserInfo::from(character)).await?;
            actor
                .send(super::MsgTalk::from_system(
                    character.id(),
                    super::TalkChannel::System,
                    format!("Equipment repaired for {cost} silver."),
                ))
                .await?;
            return Ok(());
        }
        let response = match (npc.map(|npc| npc.kind()), self.option_id) {
            (Some(crate::entities::NpcKind::ShopKeeper), 1 | 2) => {
                if let Some(offer) = _state.shop_catalog().offer(self.task_id) {
                    format!(
                        "{} has {} catalog items. Drag an item to an empty inventory slot or right-click to buy; drag an inventory item here or right-click it to sell.",
                        offer.name,
                        offer.item_ids.len(),
                    )
                } else {
                    "This shop has no catalog loaded yet.".to_owned()
                }
            },
            (Some(crate::entities::NpcKind::Storage), 1) => "Opening warehouse.".to_owned(),
            (Some(crate::entities::NpcKind::Forge), 1) => "Choose an equipped item to improve.".to_owned(),
            (Some(crate::entities::NpcKind::Face), 1) => {
                "Appearance services will be connected after the character editor.".to_owned()
            },
            (_, 1) if self.task_id == 425 => {
                format!("Training every day will make you stronger, {character_name}.")
            },
            (_, 1) if self.task_id == 10010 => "Twin City is the heart of this realm.".to_owned(),
            (_, 2) if self.task_id == 10010 => "Many secrets are hidden beyond the city gates.".to_owned(),
            (_, 1) if self.task_id == 10009 => "I heard that brave adventurers are needed near the gates.".to_owned(),
            (_, 1) if npc_name_lower.contains("guild") => {
                "A guild is stronger when its members share their skills and protect one another.".to_owned()
            },
            (_, 1)
                if npc_name_lower.contains("trainer")
                    || npc_name_lower.contains("star")
                    || npc_name_lower.contains("god")
                    || npc_name_lower.contains("general") =>
            {
                format!("Keep training, {character_name}; every battle is a lesson.")
            },
            (_, 1) => format!(
                "{} will tell you what they know, {character_name}.",
                npc_name.as_deref().unwrap_or("This NPC")
            ),
            (_, 2) => format!(
                "{} says: start by speaking with the nearby townspeople.",
                npc_name.as_deref().unwrap_or("The townspeople")
            ),
            _ => "Come back when you are ready to continue your adventure.".to_owned(),
        };
        let dialog = MsgTaskDialog::builder()
            .text(response)
            .with_option(255, "Thank you.")
            .and()
            .with_avatar(47)
            .task_id(self.task_id)
            .build();
        for message in dialog {
            actor.send(message).await?;
        }
        Ok(())
    }
}

/// Runs a teleport resolved by [`crate::teleports::resolve`]. The destination
/// cell is validated before any silver is charged, and the fee is refunded if
/// the map change itself fails.
async fn city_teleport(
    state: &crate::State,
    actor: &Actor<crate::ActorState>,
    character: &crate::entities::Character,
    npc_id: u32,
    teleport: crate::teleports::Teleport,
) -> Result<(), crate::Error> {
    use crate::teleports::{Teleport, MARKET_MAP, TWIN_CITY_CENTER, TWIN_CITY_MAP};

    let origin = character.entity().location();
    let origin_map = character.entity().map_id();
    let (map_id, position, name, fee, records_market_return) = match teleport {
        Teleport::Paid {
            destination,
            fee,
            records_market_return,
        } => (
            destination.map_id,
            destination.position,
            destination.name,
            fee,
            records_market_return,
        ),
        Teleport::Free(destination) => (destination.map_id, destination.position, destination.name, 0, false),
        Teleport::LeaveMarket => {
            let (map_id, position) = character
                .take_market_return()
                .filter(|(map_id, _)| *map_id != MARKET_MAP)
                .unwrap_or((TWIN_CITY_MAP, TWIN_CITY_CENTER));
            (map_id, position, "your previous location", 0, false)
        },
    };

    let new_map = state.try_map(map_id)?;
    new_map.load().await?;
    let mut target = origin;
    target.x = position.0;
    target.y = position.1;
    // `nearest_walkable_position` only answers for blocked cells: it returns
    // `None` when the requested cell is already free.
    let target = if new_map.is_walkable(target.x, target.y) {
        Some(target)
    } else {
        new_map.nearest_walkable_position(target, 4)
    };
    let Some(target) = target else {
        tracing::warn!(map_id, ?position, "city teleport destination is not walkable");
        actor
            .send(super::MsgTalk::from_system(
                character.id(),
                super::TalkChannel::TopLeft,
                format!("The road to {name} is closed right now."),
            ))
            .await?;
        return Ok(());
    };

    if fee > 0 {
        if character.spend_silver(state.pool(), fee).await?.is_none() {
            actor
                .send(super::MsgTalk::from_system(
                    character.id(),
                    super::TalkChannel::TopLeft,
                    format!("The trip to {name} costs {fee} silver."),
                ))
                .await?;
            return Ok(());
        }
        crate::audit::economic("npc_teleport", character.id(), npc_id, 0, fee);
    }

    let entity = actor.entity();
    let moved = async {
        character.teleport(state, map_id, (target.x, target.y)).await?;
        new_map.insert_entity(entity.clone()).await?;
        character.try_screen()?.load_surroundings(state).await
    }
    .await;
    if let Err(error) = moved {
        if fee > 0 {
            character.grant_silver(state.pool(), fee).await?;
            crate::audit::economic("npc_teleport_refund", character.id(), npc_id, 0, fee);
            actor.send(super::MsgUserInfo::from(character)).await?;
        }
        return Err(error);
    }

    if records_market_return && origin_map != MARKET_MAP {
        character.set_market_return(origin_map, (origin.x, origin.y));
    }
    if fee > 0 {
        actor.send(super::MsgUserInfo::from(character)).await?;
    }
    actor
        .send(super::MsgTalk::from_system(
            character.id(),
            super::TalkChannel::System,
            format!("Teleported to {name}."),
        ))
        .await?;
    Ok(())
}
