//! The Guild System window, drawn inside the GUILD quick panel. Without a
//! guild: what the system is (with the invitations received and the
//! requests sent), the guilds to ask to join, and founding one. In a guild:
//! its header (crest, name, title, motto, members, level) over eight tabs:
//! overview, members, requests, history, fund, wars, deaths and frags, and
//! the leader's settings. The server (`guild_panel.rs`) sends the header
//! and the lists; each click here becomes one of its requests.

use crate::components::{centered_text, draw_bar, draw_button, draw_card, draw_tab, fill, outline, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use gravak_protocol::{GuildPanelInfo, GuildPanelPacket, GuildPanelRow};
use image::RgbaImage;

/// The lists the server sends, by its numbers (`GuildPanelPacket::MEMBERS`...).
const LISTS: usize = 12;
/// Rows on a page of the server's lists; the members and requests are paged
/// here.
const PAGE_ROWS: usize = 8;
const MEMBER_ROWS: usize = 7;
const REQUEST_CARDS: usize = 2;
const INVITE_ROWS: usize = 3;
/// Founding a guild, as the server asks.
pub const FOUND_LEVEL: u32 = 90;
pub const FOUND_PRICE: u64 = 1_000_000;
/// The players' time zone (Brasília, no daylight saving).
const UTC_OFFSET_SECS: i64 = -3 * 3_600;

const LEADER: u32 = 1_000;
const DEPUTY: u32 = 990;

const GREEN: [u8; 3] = [150, 220, 140];
const RED: [u8; 3] = [230, 130, 120];
const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
const TEXT: [u8; 3] = ui_theme::TEXT;
const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    System,
    Search,
    Create,
    Overview,
    Members,
    Requests,
    History,
    Balance,
    War,
    Deaths,
    Settings,
    /// The crest picker, from the header (the leader), Settings or Create.
    Crest,
}

const OUTSIDE_TABS: [(Tab, &str); 3] = [(Tab::System, "Guild System"), (Tab::Search, "Find a Guild"), (Tab::Create, "Create a Guild")];
const INSIDE_TABS: [(Tab, &str); 8] = [
    (Tab::Overview, "Overview"),
    (Tab::Members, "Members"),
    (Tab::Requests, "Requests"),
    (Tab::History, "History"),
    (Tab::Balance, "Bank"),
    (Tab::War, "War"),
    (Tab::Deaths, "Deaths"),
    (Tab::Settings, "Settings"),
];

/// The text boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    CreateName,
    CreateTitle,
    CreateMotto,
    CreateDescription,
    Search,
    JoinMessage,
    InviteName,
    Amount,
    HistorySearch,
    DeathSearch,
    WarName,
    SetTitle,
    SetMotto,
    SetDescription,
    SetFee,
}

const FIELDS: usize = 15;

impl Field {
    fn index(self) -> usize {
        self as usize
    }

    /// How many characters it holds, and whether only digits.
    fn limit(self) -> (usize, bool) {
        match self {
            Self::CreateName => (24, false),
            Self::CreateTitle | Self::SetTitle => (30, false),
            Self::CreateMotto | Self::SetMotto => (60, false),
            Self::CreateDescription | Self::SetDescription => (250, false),
            Self::JoinMessage => (120, false),
            Self::Amount | Self::SetFee => (10, true),
            _ => (24, false),
        }
    }

    /// The box after this one in its tab (Tab).
    fn next(self) -> Self {
        match self {
            Self::CreateName => Self::CreateTitle,
            Self::CreateTitle => Self::CreateMotto,
            Self::CreateMotto => Self::CreateDescription,
            Self::CreateDescription => Self::CreateName,
            Self::SetTitle => Self::SetMotto,
            Self::SetMotto => Self::SetDescription,
            Self::SetDescription => Self::SetFee,
            Self::SetFee => Self::SetTitle,
            other => other,
        }
    }
}

/// One of the server's lists as it arrived.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct GuildList {
    pub header: GuildPanelRow,
    pub rows: Vec<GuildPanelRow>,
}

impl GuildList {
    fn total(&self) -> u32 {
        self.header.total
    }

    fn pages(&self) -> u32 {
        self.total().div_ceil(PAGE_ROWS as u32).max(1)
    }
}

/// Everything the window shows, and what was typed and chosen in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuildView {
    pub info: Option<GuildPanelInfo>,
    pub lists: Vec<GuildList>,
    pub tab: Tab,
    pub inputs: Vec<String>,
    pub focused: Option<Field>,
    /// The member, request or guild picked in a list.
    pub selected: Option<u32>,
    pub selected_war: Option<u32>,
    /// Pages of the server's lists, and of the members and requests here.
    pub pages: Vec<u32>,
    pub member_page: u32,
    pub request_page: u32,
    /// History: 0 all, 1 members, 2 fund, 3 war, 4 settings.
    pub history_filter: u8,
    /// Deaths: 0 all, 1 deaths, 2 frags; and 0 any cause, 1 players, 2 monsters.
    pub death_kind: u8,
    pub death_cause: u8,
    /// The settings' "taking requests" switch, and the guild whose settings
    /// fill the boxes.
    pub accepting: bool,
    pub settings_of: u32,
    /// A second click confirms leaving or disbanding.
    pub confirm: Option<GuildAction>,
    pub hover: Option<GuildAction>,
    /// The minute now, for "5 min ago".
    pub now: u64,
    /// The hero's own name, to know their row.
    pub hero: String,
    /// The crest being chosen, the one a new guild will have, and the tab
    /// the picker goes back to.
    pub crest_draft: u32,
    pub new_crest: u32,
    pub crest_return: Tab,
}

impl Default for GuildView {
    fn default() -> Self {
        Self {
            info: None,
            lists: vec![GuildList::default(); LISTS],
            tab: Tab::System,
            inputs: vec![String::new(); FIELDS],
            focused: None,
            selected: None,
            selected_war: None,
            pages: vec![0; LISTS],
            member_page: 0,
            request_page: 0,
            history_filter: 0,
            death_kind: 0,
            death_cause: 0,
            accepting: true,
            settings_of: 0,
            confirm: None,
            hover: None,
            now: 0,
            hero: String::new(),
            crest_draft: 0,
            new_crest: 0,
            crest_return: Tab::Overview,
        }
    }
}

/// What a click on the window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuildAction {
    Tab(Tab),
    Focus(Field),
    Select(u32),
    SelectWar(u32),
    /// A page of one of the server's lists.
    Page { list: u32, page: u32 },
    MemberPage(u32),
    RequestPage(u32),
    HistoryFilter(u8),
    DeathKind(u8),
    DeathCause(u8),
    ToggleAccepting,
    Search,
    HistorySearch,
    DeathSearch,
    Create,
    RequestJoin(u32),
    CancelRequest(u32),
    AcceptInvite(u32),
    RefuseInvite(u32),
    AcceptRequest(u32),
    RefuseRequest(u32),
    Invite,
    CancelInvite(u32),
    Leave,
    Kick(u32),
    Promote(u32),
    Demote(u32),
    Transfer(u32),
    Deposit,
    Withdraw,
    SaveSettings,
    DeclareWar,
    AnswerWar(u32, bool),
    EndWar(u32),
    Disband,
    /// Opens the guild's warehouse (main does it).
    Warehouse,
    /// Opens the crest picker.
    OpenCrest,
    PickEmblem(u8),
    PickField(u8),
    PickColour(u8),
    /// Keeps the crest: the guild's (the leader) or the new guild's.
    SaveCrest,
    CloseCrest,
}

impl GuildView {
    pub fn in_guild(&self) -> bool {
        self.info.as_ref().is_some_and(|info| info.in_guild == 1)
    }

    fn role(&self) -> u32 {
        self.info.as_ref().map_or(0, |info| info.role)
    }

    fn leader(&self) -> bool {
        self.role() >= LEADER
    }

    fn officer(&self) -> bool {
        self.role() >= DEPUTY
    }

    fn list(&self, list: u32) -> &GuildList {
        &self.lists[list as usize % LISTS]
    }

    pub fn input(&self, field: Field) -> &str {
        &self.inputs[field.index()]
    }

    fn input_mut(&mut self, field: Field) -> &mut String {
        &mut self.inputs[field.index()]
    }

    /// The header arrived: the tabs follow whether there is a guild, and the
    /// settings' boxes fill with the guild's own once.
    pub fn receive_info(&mut self, info: GuildPanelInfo) {
        let inside = info.in_guild == 1;
        let tab_inside = match self.tab {
            Tab::System | Tab::Search | Tab::Create => false,
            Tab::Crest => !matches!(self.crest_return, Tab::Create),
            _ => true,
        };
        if inside != tab_inside {
            self.tab = if inside { Tab::Overview } else { Tab::System };
            self.selected = None;
        }
        if inside && self.settings_of != info.guild_id {
            self.settings_of = info.guild_id;
            *self.input_mut(Field::SetTitle) = info.title.clone();
            *self.input_mut(Field::SetMotto) = info.motto.clone();
            *self.input_mut(Field::SetDescription) = info.description.clone();
            *self.input_mut(Field::SetFee) = if info.entry_fee > 0 { info.entry_fee.to_string() } else { String::new() };
            self.accepting = info.accepting != 0;
        }
        if !inside {
            self.settings_of = 0;
        }
        self.info = Some(info);
    }

    /// A row arrived: its header starts the list over.
    pub fn receive_row(&mut self, row: GuildPanelRow) {
        let list = &mut self.lists[row.list as usize % LISTS];
        if row.index == 0 {
            list.rows.clear();
            list.header = row;
        } else {
            list.rows.push(row);
        }
    }

    /// Typing in the box under focus.
    pub fn type_text(&mut self, text: &str) {
        let Some(field) = self.focused else {
            return;
        };
        let (limit, digits) = field.limit();
        let input = self.input_mut(field);
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if digits && !ch.is_ascii_digit() {
                continue;
            }
            if input.chars().count() < limit {
                input.push(ch);
            }
        }
    }

    pub fn erase(&mut self) {
        if let Some(field) = self.focused {
            self.input_mut(field).pop();
        }
    }

    pub fn focus_next(&mut self) {
        self.focused = self.focused.map(Field::next);
    }

    /// What Enter does in the box under focus.
    pub fn submit(&self) -> Option<GuildAction> {
        Some(match self.focused? {
            Field::CreateName | Field::CreateTitle | Field::CreateMotto | Field::CreateDescription => GuildAction::Create,
            Field::Search => GuildAction::Search,
            Field::JoinMessage => GuildAction::RequestJoin(self.selected?),
            Field::InviteName => GuildAction::Invite,
            Field::Amount => GuildAction::Deposit,
            Field::HistorySearch => GuildAction::HistorySearch,
            Field::DeathSearch => GuildAction::DeathSearch,
            Field::WarName => GuildAction::DeclareWar,
            Field::SetTitle | Field::SetMotto | Field::SetDescription | Field::SetFee => GuildAction::SaveSettings,
        })
    }

    fn amount(&self, field: Field) -> u32 {
        self.input(field).parse().unwrap_or(0)
    }

    fn history_kind(&self) -> &'static str {
        ["", "members", "fund", "war", "settings"][usize::from(self.history_filter.min(4))]
    }

    fn death_filter(&self) -> String {
        let kind = ["", "deaths", "frags"][usize::from(self.death_kind.min(2))];
        let cause = ["", "players", "monsters"][usize::from(self.death_cause.min(2))];
        format!("{kind}:{cause}")
    }

    /// A page of `list` with its filters.
    fn page_packet(&self, list: u32, page: u32) -> GuildPanelPacket {
        let (filter, search) = match list {
            GuildPanelPacket::EVENTS => (self.history_kind().to_owned(), self.input(Field::HistorySearch).to_owned()),
            GuildPanelPacket::DEATHS => (self.death_filter(), self.input(Field::DeathSearch).to_owned()),
            GuildPanelPacket::SEARCH => (String::new(), self.input(Field::Search).to_owned()),
            GuildPanelPacket::WAR_KILLS => (self.selected_war.unwrap_or(0).to_string(), String::new()),
            _ => (String::new(), String::new()),
        };
        GuildPanelPacket::new(GuildPanelPacket::PAGE, list, page).with_texts([&filter, &search, "", ""])
    }

    /// Does what a click asks on this side, and the request it sends, if
    /// any. Leaving and disbanding ask a second click.
    pub fn apply(&mut self, action: GuildAction) -> Option<GuildPanelPacket> {
        use GuildPanelPacket as P;
        let confirmed = self.confirm == Some(action);
        self.confirm = None;
        if !matches!(action, GuildAction::Focus(_)) {
            self.focused = None;
        }
        let simple = |kind: u32, param: u32| Some(P::new(kind, param, 0));
        match action {
            GuildAction::Tab(tab) => {
                self.tab = tab;
                self.selected = None;
                None
            },
            GuildAction::Focus(field) => {
                self.focused = Some(field);
                None
            },
            GuildAction::Select(id) => {
                self.selected = (self.selected != Some(id)).then_some(id);
                None
            },
            GuildAction::SelectWar(id) => {
                self.selected_war = Some(id);
                self.pages[P::WAR_KILLS as usize] = 0;
                Some(self.page_packet(P::WAR_KILLS, 0))
            },
            GuildAction::Page { list, page } => {
                self.pages[list as usize % LISTS] = page;
                Some(self.page_packet(list, page))
            },
            GuildAction::MemberPage(page) => {
                self.member_page = page;
                None
            },
            GuildAction::RequestPage(page) => {
                self.request_page = page;
                None
            },
            GuildAction::HistoryFilter(filter) => {
                self.history_filter = filter;
                self.pages[P::EVENTS as usize] = 0;
                Some(self.page_packet(P::EVENTS, 0))
            },
            GuildAction::DeathKind(kind) => {
                self.death_kind = kind;
                self.pages[P::DEATHS as usize] = 0;
                Some(self.page_packet(P::DEATHS, 0))
            },
            GuildAction::DeathCause(cause) => {
                self.death_cause = cause;
                self.pages[P::DEATHS as usize] = 0;
                Some(self.page_packet(P::DEATHS, 0))
            },
            GuildAction::Search => {
                self.pages[P::SEARCH as usize] = 0;
                Some(self.page_packet(P::SEARCH, 0))
            },
            GuildAction::HistorySearch => {
                self.pages[P::EVENTS as usize] = 0;
                Some(self.page_packet(P::EVENTS, 0))
            },
            GuildAction::DeathSearch => {
                self.pages[P::DEATHS as usize] = 0;
                Some(self.page_packet(P::DEATHS, 0))
            },
            GuildAction::ToggleAccepting => {
                self.accepting = !self.accepting;
                None
            },
            GuildAction::Create => Some(P::new(P::CREATE, 0, self.new_crest).with_texts([
                self.input(Field::CreateName),
                self.input(Field::CreateTitle),
                self.input(Field::CreateMotto),
                self.input(Field::CreateDescription),
            ])),
            GuildAction::RequestJoin(guild) => {
                let message = self.input(Field::JoinMessage).to_owned();
                self.input_mut(Field::JoinMessage).clear();
                Some(P::new(P::REQUEST_JOIN, guild, 0).with_texts([&message, "", "", ""]))
            },
            GuildAction::CancelRequest(guild) => simple(P::CANCEL_REQUEST, guild),
            GuildAction::AcceptInvite(guild) => simple(P::ACCEPT_INVITE, guild),
            GuildAction::RefuseInvite(guild) => simple(P::REFUSE_INVITE, guild),
            GuildAction::AcceptRequest(id) => simple(P::ACCEPT_REQUEST, id),
            GuildAction::RefuseRequest(id) => simple(P::REFUSE_REQUEST, id),
            GuildAction::Invite => {
                let name = self.input(Field::InviteName).trim().to_owned();
                self.input_mut(Field::InviteName).clear();
                (!name.is_empty()).then(|| P::new(P::INVITE, 0, 0).with_texts([&name, "", "", ""]))
            },
            GuildAction::CancelInvite(id) => simple(P::CANCEL_INVITE, id),
            GuildAction::Leave | GuildAction::Disband if !confirmed => {
                self.confirm = Some(action);
                None
            },
            GuildAction::Leave => simple(P::LEAVE, 0),
            GuildAction::Disband => simple(P::DISBAND, 0),
            GuildAction::Kick(id) => simple(P::KICK, id),
            GuildAction::Promote(id) => simple(P::PROMOTE, id),
            GuildAction::Demote(id) => simple(P::DEMOTE, id),
            GuildAction::Transfer(_) if !confirmed => {
                self.confirm = Some(action);
                None
            },
            GuildAction::Transfer(id) => simple(P::TRANSFER, id),
            GuildAction::Deposit | GuildAction::Withdraw => {
                let amount = self.amount(Field::Amount);
                self.input_mut(Field::Amount).clear();
                let kind = if action == GuildAction::Deposit { P::DEPOSIT } else { P::WITHDRAW };
                (amount > 0).then(|| P::new(kind, 0, amount))
            },
            GuildAction::SaveSettings => Some(
                P::new(P::SETTINGS, u32::from(self.accepting), self.amount(Field::SetFee)).with_texts([
                    self.input(Field::SetTitle),
                    self.input(Field::SetMotto),
                    self.input(Field::SetDescription),
                    "",
                ]),
            ),
            GuildAction::DeclareWar => {
                let name = self.input(Field::WarName).trim().to_owned();
                self.input_mut(Field::WarName).clear();
                (!name.is_empty()).then(|| P::new(P::DECLARE_WAR, 0, 0).with_texts([&name, "", "", ""]))
            },
            GuildAction::AnswerWar(war, accept) => Some(P::new(P::ANSWER_WAR, war, u32::from(accept))),
            GuildAction::EndWar(war) => simple(P::END_WAR, war),
            GuildAction::Warehouse => None,
            GuildAction::OpenCrest => {
                self.crest_return = if self.tab == Tab::Crest { self.crest_return } else { self.tab };
                self.crest_draft = if self.in_guild() {
                    self.info.as_ref().map_or(0, |info| info.emblem)
                } else {
                    self.new_crest
                };
                self.tab = Tab::Crest;
                None
            },
            GuildAction::PickEmblem(icon) => {
                let draft = self.crest_draft;
                self.crest_draft = crate::guild_emblems::compose(
                    usize::from(icon),
                    crate::guild_emblems::field_of(draft),
                    crate::guild_emblems::colour_of(draft),
                );
                None
            },
            GuildAction::PickField(field) => {
                let draft = self.crest_draft;
                self.crest_draft = crate::guild_emblems::compose(
                    crate::guild_emblems::icon_of(draft),
                    usize::from(field),
                    crate::guild_emblems::colour_of(draft),
                );
                None
            },
            GuildAction::PickColour(colour) => {
                let draft = self.crest_draft;
                self.crest_draft = crate::guild_emblems::compose(
                    crate::guild_emblems::icon_of(draft),
                    crate::guild_emblems::field_of(draft),
                    usize::from(colour),
                );
                None
            },
            GuildAction::CloseCrest => {
                self.tab = self.crest_return;
                None
            },
            GuildAction::SaveCrest => {
                self.tab = self.crest_return;
                if self.in_guild() {
                    Some(P::new(P::EMBLEM, self.crest_draft, 0))
                } else {
                    self.new_crest = self.crest_draft;
                    None
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

fn outside_tab(index: usize) -> UiRect {
    UiRect::new(16 + index as i32 * 164, 64, 160, 22)
}

fn inside_tab(index: usize) -> UiRect {
    UiRect::new(16 + index as i32 * 61, 102, 58, 20)
}

/// Where the picker's content starts: under the header in a guild, under
/// the tabs without one.
fn crest_top(view: &GuildView) -> i32 {
    if view.in_guild() { 128 } else { 94 }
}

const CREST_COLUMNS: usize = 9;

fn emblem_cell(view: &GuildView, index: usize) -> UiRect {
    UiRect::new(
        112 + (index % CREST_COLUMNS) as i32 * 42,
        crest_top(view) + (index / CREST_COLUMNS) as i32 * 40,
        40,
        38,
    )
}

fn field_swatch(view: &GuildView, index: usize) -> UiRect {
    UiRect::new(112 + index as i32 * 30, crest_top(view) + 166, 26, 20)
}

fn colour_swatch(view: &GuildView, index: usize) -> UiRect {
    UiRect::new(112 + index as i32 * 30, crest_top(view) + 192, 26, 20)
}

/// The header's crest, which the leader clicks to change it.
const HEADER_CREST: UiRect = UiRect::new(16, 30, 60, 66);

fn pager(y: i32) -> [UiRect; 2] {
    [UiRect::new(180, y, 30, 18), UiRect::new(310, y, 30, 18)]
}

fn field_rect(field: Field) -> UiRect {
    match field {
        Field::CreateName => UiRect::new(24, 126, 472, 22),
        Field::CreateTitle => UiRect::new(24, 164, 472, 22),
        Field::CreateMotto => UiRect::new(24, 202, 472, 22),
        Field::CreateDescription => UiRect::new(24, 240, 472, 40),
        Field::Search => UiRect::new(24, 98, 372, 22),
        Field::JoinMessage => UiRect::new(24, 346, 330, 22),
        Field::InviteName => UiRect::new(24, 132, 300, 22),
        Field::Amount => UiRect::new(24, 184, 180, 22),
        Field::HistorySearch | Field::DeathSearch => UiRect::new(24, 154, 372, 22),
        Field::WarName => UiRect::new(20, 344, 116, 22),
        Field::SetTitle => UiRect::new(24, 144, 472, 22),
        Field::SetMotto => UiRect::new(24, 182, 472, 22),
        Field::SetDescription => UiRect::new(24, 220, 472, 40),
        Field::SetFee => UiRect::new(24, 280, 140, 22),
    }
}

/// The buttons each tab has, with the action and whether it can be used.
fn buttons(view: &GuildView) -> Vec<(UiRect, &'static str, GuildAction, bool)> {
    use GuildAction as A;
    let mut list = Vec::new();
    let info = view.info.clone().unwrap_or_default();
    match view.tab {
        Tab::System => {
            let invites = &view.list(GuildPanelPacket::MY_INVITES).rows;
            for (index, invite) in invites.iter().take(3).enumerate() {
                let y = 200 + index as i32 * 24;
                list.push((UiRect::new(332, y, 76, 20), "Refuse", A::RefuseInvite(invite.id), true));
                list.push((UiRect::new(414, y, 82, 20), "Accept", A::AcceptInvite(invite.id), true));
            }
            let requests = &view.list(GuildPanelPacket::MY_REQUESTS).rows;
            for (index, request) in requests.iter().take(3).enumerate() {
                let y = 296 + index as i32 * 20;
                list.push((UiRect::new(414, y, 82, 18), "Cancel", A::CancelRequest(request.id), true));
            }
        },
        Tab::Search => {
            list.push((UiRect::new(402, 98, 94, 22), "Search", A::Search, true));
            let search = view.list(GuildPanelPacket::SEARCH);
            let page = view.pages[GuildPanelPacket::SEARCH as usize];
            push_pager(&mut list, 322, GuildPanelPacket::SEARCH, page, search.pages());
            if let Some(guild) = view.selected.and_then(|id| search.rows.iter().find(|row| row.id == id)) {
                if guild.extra & 2 != 0 {
                    list.push((UiRect::new(362, 346, 134, 22), "Cancel request", A::CancelRequest(guild.id), true));
                } else {
                    list.push((UiRect::new(362, 346, 134, 22), "Ask to join", A::RequestJoin(guild.id), guild.extra & 1 != 0));
                }
            }
        },
        Tab::Create => {
            let able = info.hero_level >= FOUND_LEVEL && info.silver >= FOUND_PRICE && view.input(Field::CreateName).trim().chars().count() >= 3;
            list.push((UiRect::new(346, 344, 150, 24), "Pay and confirm", A::Create, able));
            list.push((UiRect::new(186, 344, 150, 24), "Choose crest", A::OpenCrest, true));
        },
        Tab::Crest => {
            list.push((UiRect::new(24, 346, 100, 22), "Back", A::CloseCrest, true));
            let label = if view.in_guild() { "Save crest" } else { "Use this crest" };
            list.push((UiRect::new(356, 346, 140, 22), label, A::SaveCrest, !view.in_guild() || view.leader()));
        },
        Tab::Overview => {
            let label = if view.confirm == Some(A::Leave) { "Confirm leaving" } else { "Leave guild" };
            let alone = info.members <= 1;
            list.push((UiRect::new(24, 346, 140, 22), label, A::Leave, !view.leader() || alone));
            list.push((UiRect::new(356, 346, 140, 22), "Guild warehouse", A::Warehouse, true));
        },
        Tab::Members => {
            let members = &view.list(GuildPanelPacket::MEMBERS).rows;
            let pages = (members.len().div_ceil(MEMBER_ROWS) as u32).max(1);
            if view.member_page > 0 {
                list.push((pager(310)[0], "<", A::MemberPage(view.member_page - 1), true));
            }
            if view.member_page + 1 < pages {
                list.push((pager(310)[1], ">", A::MemberPage(view.member_page + 1), true));
            }
            if let Some(member) = view.selected.and_then(|id| members.iter().find(|row| row.id == id)) {
                let own = member.text1 == view.hero;
                let ranked = member.value >= DEPUTY;
                if view.leader() && !own {
                    if ranked {
                        list.push((UiRect::new(24, 346, 92, 22), "Demote", A::Demote(member.id), true));
                    } else {
                        list.push((UiRect::new(24, 346, 92, 22), "Promote", A::Promote(member.id), true));
                    }
                    let label = if view.confirm == Some(A::Transfer(member.id)) { "Confirm" } else { "Pass leadership" };
                    list.push((UiRect::new(220, 346, 140, 22), label, A::Transfer(member.id), true));
                }
                if view.officer() && !own && (view.leader() || !ranked) {
                    list.push((UiRect::new(122, 346, 92, 22), "Kick", A::Kick(member.id), true));
                }
            }
        },
        Tab::Requests => {
            list.push((UiRect::new(330, 132, 90, 22), "Invite", A::Invite, view.officer()));
            for (index, invite) in view.list(GuildPanelPacket::INVITES).rows.iter().take(INVITE_ROWS).enumerate() {
                list.push((UiRect::new(414, 176 + index as i32 * 19, 82, 17), "Cancel", A::CancelInvite(invite.id), view.officer()));
            }
            let requests = &view.list(GuildPanelPacket::REQUESTS).rows;
            let start = view.request_page as usize * REQUEST_CARDS;
            for (index, request) in requests.iter().skip(start).take(REQUEST_CARDS).enumerate() {
                let y = 254 + index as i32 * 46;
                list.push((UiRect::new(334, y + 22, 76, 18), "Refuse", A::RefuseRequest(request.id), view.officer()));
                list.push((UiRect::new(414, y + 22, 82, 18), "Accept", A::AcceptRequest(request.id), view.officer()));
            }
            let pages = (requests.len().div_ceil(REQUEST_CARDS) as u32).max(1);
            if view.request_page > 0 {
                list.push((pager(348)[0], "<", A::RequestPage(view.request_page - 1), true));
            }
            if view.request_page + 1 < pages {
                list.push((pager(348)[1], ">", A::RequestPage(view.request_page + 1), true));
            }
        },
        Tab::History => {
            for (index, label) in ["All", "Members", "Bank", "War", "Settings"].into_iter().enumerate() {
                let _ = label;
                list.push((UiRect::new(24 + index as i32 * 95, 128, 91, 20), "", A::HistoryFilter(index as u8), true));
            }
            list.push((UiRect::new(402, 154, 94, 22), "Search", A::HistorySearch, true));
            let events = view.list(GuildPanelPacket::EVENTS);
            push_pager(&mut list, 330, GuildPanelPacket::EVENTS, view.pages[GuildPanelPacket::EVENTS as usize], events.pages());
        },
        Tab::Balance => {
            list.push((UiRect::new(212, 184, 100, 22), "Deposit", A::Deposit, true));
            list.push((UiRect::new(318, 184, 100, 22), "Withdraw", A::Withdraw, view.leader()));
            let balance = view.list(GuildPanelPacket::BALANCE);
            push_pager(&mut list, 340, GuildPanelPacket::BALANCE, view.pages[GuildPanelPacket::BALANCE as usize], balance.pages());
        },
        Tab::War => {
            list.push((UiRect::new(140, 344, 80, 22), "Declare", A::DeclareWar, view.leader()));
            if let Some(war) = view.selected_war.and_then(|id| view.list(GuildPanelPacket::WARS).rows.iter().find(|row| row.id == id)) {
                match war.value {
                    0 => {
                        list.push((UiRect::new(236, 182, 90, 22), "Accept", A::AnswerWar(war.id, true), view.leader()));
                        list.push((UiRect::new(332, 182, 90, 22), "Refuse", A::AnswerWar(war.id, false), view.leader()));
                    },
                    1 => list.push((UiRect::new(236, 182, 130, 22), "Withdraw declaration", A::AnswerWar(war.id, false), view.leader())),
                    2 => list.push((UiRect::new(236, 182, 130, 22), "End war", A::EndWar(war.id), view.leader())),
                    _ => {},
                }
                let kills = view.list(GuildPanelPacket::WAR_KILLS);
                push_pager_at(&mut list, [268, 344], GuildPanelPacket::WAR_KILLS, view.pages[GuildPanelPacket::WAR_KILLS as usize], kills.pages());
            }
        },
        Tab::Deaths => {
            for index in 0..3 {
                list.push((UiRect::new(24 + index * 76, 128, 72, 20), "", A::DeathKind(index as u8), true));
                list.push((UiRect::new(268 + index * 76, 128, 72, 20), "", A::DeathCause(index as u8), true));
            }
            list.push((UiRect::new(402, 154, 94, 22), "Search", A::DeathSearch, true));
            let deaths = view.list(GuildPanelPacket::DEATHS);
            push_pager(&mut list, 344, GuildPanelPacket::DEATHS, view.pages[GuildPanelPacket::DEATHS as usize], deaths.pages());
        },
        Tab::Settings => {
            if view.leader() {
                let accepting = if view.accepting { "Taking requests: Yes" } else { "Taking requests: No" };
                list.push((UiRect::new(180, 280, 170, 22), accepting, A::ToggleAccepting, true));
                list.push((UiRect::new(356, 280, 140, 22), "Save", A::SaveSettings, true));
                let label = if view.confirm == Some(A::Disband) { "Confirm disbanding" } else { "Disband guild" };
                list.push((UiRect::new(24, 346, 160, 22), label, A::Disband, true));
                list.push((UiRect::new(356, 346, 140, 22), "Change crest", A::OpenCrest, true));
            }
        },
    }
    list
}

fn push_pager(list: &mut Vec<(UiRect, &'static str, GuildAction, bool)>, y: i32, which: u32, page: u32, pages: u32) {
    let [previous, next] = pager(y);
    if page > 0 {
        list.push((previous, "<", GuildAction::Page { list: which, page: page - 1 }, true));
    }
    if page + 1 < pages {
        list.push((next, ">", GuildAction::Page { list: which, page: page + 1 }, true));
    }
}

fn push_pager_at(list: &mut Vec<(UiRect, &'static str, GuildAction, bool)>, [x, y]: [i32; 2], which: u32, page: u32, pages: u32) {
    if page > 0 {
        list.push((UiRect::new(x, y, 30, 18), "<", GuildAction::Page { list: which, page: page - 1 }, true));
    }
    if page + 1 < pages {
        list.push((UiRect::new(x + 170, y, 30, 18), ">", GuildAction::Page { list: which, page: page + 1 }, true));
    }
}

/// The text boxes each tab has.
fn fields(view: &GuildView) -> &'static [Field] {
    match view.tab {
        Tab::Search => &[Field::Search, Field::JoinMessage],
        Tab::Create => &[Field::CreateName, Field::CreateTitle, Field::CreateMotto, Field::CreateDescription],
        Tab::Requests => &[Field::InviteName],
        Tab::Balance => &[Field::Amount],
        Tab::History => &[Field::HistorySearch],
        Tab::Deaths => &[Field::DeathSearch],
        Tab::War => &[Field::WarName],
        Tab::Settings if view.leader() => &[Field::SetTitle, Field::SetMotto, Field::SetDescription, Field::SetFee],
        _ => &[],
    }
}

/// The rows a click picks: members, guilds found, wars.
fn row_rects(view: &GuildView) -> Vec<(UiRect, GuildAction)> {
    let mut rects = Vec::new();
    match view.tab {
        Tab::Members => {
            let start = view.member_page as usize * MEMBER_ROWS;
            for (index, member) in view.list(GuildPanelPacket::MEMBERS).rows.iter().skip(start).take(MEMBER_ROWS).enumerate() {
                rects.push((UiRect::new(20, 150 + index as i32 * 22, 480, 21), GuildAction::Select(member.id)));
            }
        },
        Tab::Search => {
            for (index, guild) in view.list(GuildPanelPacket::SEARCH).rows.iter().take(PAGE_ROWS).enumerate() {
                rects.push((UiRect::new(20, 126 + index as i32 * 24, 480, 23), GuildAction::Select(guild.id)));
            }
        },
        Tab::Crest => {
            for index in 0..crate::guild_emblems::EMBLEMS.len() {
                rects.push((emblem_cell(view, index), GuildAction::PickEmblem(index as u8)));
            }
            for index in 0..crate::guild_emblems::FIELD_COLOURS.len() {
                rects.push((field_swatch(view, index), GuildAction::PickField(index as u8)));
            }
            for index in 0..crate::guild_emblems::EMBLEM_COLOURS.len() {
                rects.push((colour_swatch(view, index), GuildAction::PickColour(index as u8)));
            }
        },
        Tab::War => {
            for (index, war) in view.list(GuildPanelPacket::WARS).rows.iter().take(8).enumerate() {
                rects.push((UiRect::new(20, 150 + index as i32 * 23, 200, 22), GuildAction::SelectWar(war.id)));
            }
        },
        _ => {},
    }
    rects
}

/// The action under `cursor`, in the panel's own coordinates.
pub fn action_at(view: &GuildView, cursor: [f32; 2]) -> Option<GuildAction> {
    if view.in_guild() && view.leader() && view.tab != Tab::Crest && HEADER_CREST.contains_local(cursor) {
        return Some(GuildAction::OpenCrest);
    }
    if view.in_guild() {
        if let Some(index) = (0..INSIDE_TABS.len()).find(|index| inside_tab(*index).contains_local(cursor)) {
            return Some(GuildAction::Tab(INSIDE_TABS[index].0));
        }
    } else if let Some(index) = (0..OUTSIDE_TABS.len()).find(|index| outside_tab(*index).contains_local(cursor)) {
        return Some(GuildAction::Tab(OUTSIDE_TABS[index].0));
    }
    if let Some((_, _, action, _)) = buttons(view).into_iter().find(|(rect, _, _, able)| *able && rect.contains_local(cursor)) {
        return Some(action);
    }
    if let Some(field) = fields(view).iter().find(|field| field_rect(**field).contains_local(cursor)) {
        return Some(GuildAction::Focus(*field));
    }
    row_rects(view).into_iter().find(|(rect, _)| rect.contains_local(cursor)).map(|(_, action)| action)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn width_of(font: &Font, text: &str, size: f32) -> f32 {
    text.chars().map(|ch| font.metrics(ch, size).advance_width).sum()
}

fn say(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32) {
    labels::text(image, font, &[(text, colour)], [x as f32, y as f32]);
}

fn small(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32) {
    labels::text_sized(image, font, &[(text, colour)], [x as f32, y as f32], 11.0);
}

/// `text` cut to `chars` characters.
fn clip(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        text.to_owned()
    } else {
        format!("{}...", text.chars().take(chars.saturating_sub(3)).collect::<String>())
    }
}

/// `text` broken into lines of `width` pixels at `size`.
fn wrap(font: &Font, text: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if width_of(font, &candidate, size) > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `1234567` as `1,234,567`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A moment as "dd/mm/yyyy hh:mm" in the players' time zone.
pub fn date(at: u64) -> String {
    let local = at as i64 + UTC_OFFSET_SECS;
    let days = local.div_euclid(86_400);
    let seconds = local.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02}/{month:02}/{year} {:02}:{:02}", seconds / 3_600, seconds % 3_600 / 60)
}

/// How long ago, from the minute `now` ("just now", "5 min", "3 h", "2 d").
pub fn ago(at: u64, now_minute: u64) -> String {
    let minutes = now_minute.saturating_sub(at / 60);
    match minutes {
        0 => "just now".to_owned(),
        1..=59 => format!("{minutes} min ago"),
        60..=1_439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1_440),
    }
}

fn when(view: &GuildView, at: u64) -> String {
    format!("{} - {}", ago(at, view.now), date(at))
}

fn rank_name(role: u32) -> &'static str {
    match role {
        LEADER.. => "Leader",
        DEPUTY.. => "Deputy Leader",
        _ => "Member",
    }
}

fn hovered(view: &GuildView, action: GuildAction) -> bool {
    view.hover == Some(action)
}

fn draw_field(image: &mut RgbaImage, font: &Font, view: &GuildView, field: Field, placeholder: &str) {
    let rect = field_rect(field);
    let focused = view.focused == Some(field);
    fill(image, rect, [6, 16, 22, 240]);
    outline(image, rect, if focused { ui_theme::PANEL_HIGHLIGHT } else { [70, 88, 96, 220] });
    let text = view.input(field);
    if text.is_empty() && !focused {
        small(image, font, placeholder, [110, 110, 100], rect.x + 6, rect.y + 5);
        return;
    }
    let width = (rect.width - 12) as f32;
    if rect.height > 30 {
        let mut lines = wrap(font, text, width, 11.0);
        if focused {
            match lines.last_mut() {
                Some(last) => last.push('|'),
                None => lines.push("|".to_owned()),
            }
        }
        for (index, line) in lines.iter().take(((rect.height - 6) / 13) as usize).enumerate() {
            small(image, font, line, TEXT, rect.x + 6, rect.y + 3 + index as i32 * 13);
        }
        return;
    }
    let mut shown = format!("{text}{}", if focused { "|" } else { "" });
    while width_of(font, &shown, 12.0) > width && shown.chars().count() > 1 {
        shown.remove(0);
    }
    labels::text_sized(image, font, &[(shown.as_str(), TEXT)], [rect.x as f32 + 6.0, rect.y as f32 + 4.0], 12.0);
}

fn draw_buttons(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    for (rect, label, action, able) in buttons(view) {
        if label.is_empty() {
            continue;
        }
        let state = if !able {
            WidgetState::Disabled
        } else if hovered(view, action) {
            WidgetState::Hovered
        } else {
            WidgetState::Normal
        };
        draw_button(image, font, rect, label, state);
    }
}

fn page_label(image: &mut RgbaImage, font: &Font, y: i32, page: u32, pages: u32) {
    centered_text(image, font, &format!("Page {} of {}", page + 1, pages.max(1)), 260.0, y as f32 + 3.0, 11.0, MUTED);
}

fn draw_header(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let Some(info) = &view.info else {
        return;
    };
    if info.in_guild == 1 {
        crate::guild_emblems::draw_crest(image, info.emblem, HEADER_CREST.x, HEADER_CREST.y, HEADER_CREST.width as u32, HEADER_CREST.height as u32);
        if view.leader() && hovered(view, GuildAction::OpenCrest) {
            outline(image, UiRect::new(HEADER_CREST.x - 2, HEADER_CREST.y - 2, HEADER_CREST.width + 4, HEADER_CREST.height + 4), ui_theme::PANEL_HIGHLIGHT);
        }
        labels::text_sized(image, font, &[(info.name.as_str(), GOLD)], [86.0, 30.0], 15.0);
        let role = format!("Your rank: {}", rank_name(info.role));
        let width = width_of(font, &role, 11.0);
        small(image, font, &role, GOLD, 504 - width as i32, 33);
        let tagline = match (info.title.is_empty(), info.motto.is_empty()) {
            (false, false) => format!("{} - \"{}\"", info.title, info.motto),
            (false, true) => info.title.clone(),
            (true, false) => format!("\"{}\"", info.motto),
            (true, true) => String::new(),
        };
        small(image, font, &clip(&tagline, 70), TEXT, 86, 50);
        small(
            image,
            font,
            &format!("{} / {} members - {} online", info.members, info.max_members, info.online),
            MUTED,
            86,
            65,
        );
        let bar = UiRect::new(86, 81, 418, 13);
        draw_bar(
            image,
            bar,
            u32::try_from(info.experience.min(u64::from(u32::MAX))).unwrap_or(u32::MAX),
            u32::try_from(info.next_experience.min(u64::from(u32::MAX))).unwrap_or(u32::MAX).max(1),
            [176, 130, 48, 255],
            [20, 18, 14, 255],
            [120, 88, 40, 255],
        );
        let percent = info.experience * 100 / info.next_experience.max(1);
        centered_text(image, font, &format!("Level {} - {percent}% to the next", info.level), 295.0, 81.0, 10.0, TEXT);
        for (index, (tab, label)) in INSIDE_TABS.iter().enumerate() {
            let label = if *tab == Tab::Requests && info.pending_requests > 0 {
                format!("{label} ({})", info.pending_requests)
            } else {
                (*label).to_owned()
            };
            draw_tab(image, font, inside_tab(index), &label, view.tab == *tab, hovered(view, GuildAction::Tab(*tab)));
        }
    } else {
        say(image, font, "You do not belong to a guild yet", GOLD, 18, 30);
        small(image, font, "Join an existing guild or create a new one.", MUTED, 18, 46);
        let count = format!("{} guilds on the server - {} taking requests", info.guilds, info.guilds_accepting);
        let width = width_of(font, &count, 11.0);
        small(image, font, &count, MUTED, 504 - width as i32, 33);
        for (index, (tab, label)) in OUTSIDE_TABS.iter().enumerate() {
            draw_tab(image, font, outside_tab(index), label, view.tab == *tab, hovered(view, GuildAction::Tab(*tab)));
        }
    }
}

fn event_text(row: &GuildPanelRow) -> String {
    let (who, target, amount) = (&row.text2, &row.text3, thousands(u64::from(row.value)));
    match row.text1.as_str() {
        "founded" => format!("Guild founded by {who}"),
        "joined" if target == "invitation" => format!("{who} joined the guild (invitation)"),
        "joined" if !target.is_empty() => format!("{who} joined the guild (accepted by {target})"),
        "joined" => format!("{who} joined the guild"),
        "left" => format!("{who} left the guild"),
        "kicked" => format!("{who} was kicked by {target}"),
        "promoted" => format!("{who} became a Deputy Leader"),
        "demoted" => format!("{who} is a Member again"),
        "leader" => format!("{who} now leads the guild"),
        "deposit" => format!("{who} deposited {amount} silver"),
        "withdraw" => format!("{who} withdrew {amount} silver"),
        "fee" => format!("{who} paid the {amount} silver entry fee"),
        "settings" => format!("{who} changed the settings"),
        "war_declared" => format!("War declared: {who} against {target}"),
        "war_started" => format!("War begun between {who} and {target}"),
        "war_refused" => format!("War refused ({who}, {target})"),
        "war_ended" => format!("War ended ({who}, {target})"),
        other => format!("{other}: {who} {target}"),
    }
}

fn event_colour(kind: &str) -> [u8; 3] {
    match kind {
        "founded" | "joined" | "promoted" | "leader" => GREEN,
        "left" | "kicked" | "demoted" => RED,
        "deposit" | "withdraw" | "fee" => GOLD,
        kind if kind.starts_with("war") => [230, 160, 90],
        _ => TEXT,
    }
}

fn draw_system(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let lines = [
        "A guild gathers allies under one name, with a fund and a history.",
        "Find a guild and ask to join it, with a message to its leader,",
        "or found your own (level 90 and 1,000,000 silver).",
        "At war, two guilds count each other's deaths.",
    ];
    for (index, line) in lines.iter().enumerate() {
        small(image, font, line, TEXT, 24, 100 + index as i32 * 16);
    }
    let invites = &view.list(GuildPanelPacket::MY_INVITES).rows;
    say(image, font, &format!("Invitations received ({})", invites.len()), GOLD, 24, 178);
    if invites.is_empty() {
        small(image, font, "No invitations.", MUTED, 24, 200);
    }
    for (index, invite) in invites.iter().take(3).enumerate() {
        let y = 200 + index as i32 * 24;
        draw_card(image, UiRect::new(20, y - 2, 480, 22), WidgetState::Normal);
        small(image, font, &format!("{} - invited by {}", invite.text1, invite.text2), TEXT, 28, y + 3);
        small(image, font, &ago(invite.at, view.now), MUTED, 260, y + 3);
    }
    let requests = &view.list(GuildPanelPacket::MY_REQUESTS).rows;
    say(image, font, &format!("Your requests to join ({})", requests.len()), GOLD, 24, 274);
    if requests.is_empty() {
        small(image, font, "No requests.", MUTED, 24, 296);
    }
    for (index, request) in requests.iter().take(3).enumerate() {
        let y = 296 + index as i32 * 20;
        small(image, font, &format!("{} - {}", request.text1, when(view, request.at)), TEXT, 28, y + 3);
    }
}

fn draw_search(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    draw_field(image, font, view, Field::Search, "Guild name...");
    let search = view.list(GuildPanelPacket::SEARCH);
    if search.rows.is_empty() {
        small(image, font, "No guild found.", MUTED, 24, 132);
    }
    for (index, guild) in search.rows.iter().take(PAGE_ROWS).enumerate() {
        let rect = UiRect::new(20, 126 + index as i32 * 24, 480, 23);
        let selected = view.selected == Some(guild.id);
        draw_card(image, rect, if selected { WidgetState::Selected } else { WidgetState::Normal });
        crate::guild_emblems::draw_crest(image, guild.at as u32, rect.x + 3, rect.y + 1, 18, 21);
        small(image, font, &clip(&guild.text1, 22), GOLD, rect.x + 26, rect.y);
        let line = format!("Lv {} - {} members - leader {}", guild.level, guild.value, guild.text3);
        labels::text_sized(image, font, &[(clip(&line, 40).as_str(), MUTED)], [rect.x as f32 + 26.0, rect.y as f32 + 11.0], 10.0);
        small(image, font, &clip(&guild.text2, 26), TEXT, rect.x + 220, rect.y);
        labels::text_sized(image, font, &[(clip(&guild.text4, 30).as_str(), MUTED)], [rect.x as f32 + 220.0, rect.y as f32 + 11.0], 10.0);
        let (status, colour) = if guild.extra & 2 != 0 {
            ("Request sent", GOLD)
        } else if guild.extra & 1 != 0 {
            ("Taking requests", GREEN)
        } else {
            ("Closed", RED)
        };
        small(image, font, status, colour, rect.x + 392, rect.y + 5);
    }
    page_label(image, font, 322, view.pages[GuildPanelPacket::SEARCH as usize], search.pages());
    if view.selected.is_some() {
        draw_field(image, font, view, Field::JoinMessage, "Message to the leader (optional)");
    } else {
        small(image, font, "Pick a guild to ask to join it.", MUTED, 24, 350);
    }
}

fn draw_create(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    say(image, font, "Create a new guild and begin your empire!", GOLD, 24, 96);
    let labels_at = [
        (Field::CreateName, "Guild name (unique on the server, cannot be changed)", "Name"),
        (Field::CreateTitle, "Guild title (a short subtitle)", "Title"),
        (Field::CreateMotto, "Guild motto (a short phrase, shown in the window)", "Motto"),
        (Field::CreateDescription, "Guild description (the full presentation)", "Description"),
    ];
    for (field, label, placeholder) in labels_at {
        let rect = field_rect(field);
        small(image, font, label, TEXT, rect.x, rect.y - 15);
        draw_field(image, font, view, field, placeholder);
    }
    let info = view.info.clone().unwrap_or_default();
    let card = UiRect::new(20, 288, 424, 50);
    draw_card(image, card, WidgetState::Normal);
    small(image, font, "Cost: 1,000,000 silver and level 90. This cannot be undone.", GOLD, 28, 294);
    crate::guild_emblems::draw_crest(image, view.new_crest, 452, 290, 40, 46);
    let colour = if info.silver >= FOUND_PRICE && info.hero_level >= FOUND_LEVEL { GREEN } else { RED };
    small(image, font, &format!("Your silver: {} - your level: {}", thousands(info.silver), info.hero_level), colour, 28, 314);
}

fn draw_overview(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let Some(info) = &view.info else {
        return;
    };
    let card = UiRect::new(20, 128, 480, 82);
    draw_card(image, card, WidgetState::Normal);
    say(image, font, "About the guild", GOLD, 28, 132);
    let description = if info.description.is_empty() { "The leader has not written a description yet." } else { &info.description };
    for (index, line) in wrap(font, description, 460.0, 11.0).iter().take(3).enumerate() {
        small(image, font, line, TEXT, 28, 152 + index as i32 * 15);
    }
    let rows = [
        ("Leader", info.leader.clone()),
        ("Founded", date(info.created_at)),
        ("Members", format!("{} / {} ({} online)", info.members, info.max_members, info.online)),
        ("Fund", format!("{} silver", thousands(info.fund))),
        ("Entry fee", if info.entry_fee > 0 { format!("{} silver", thousands(u64::from(info.entry_fee))) } else { "none".to_owned() }),
        ("Requests to join", if info.accepting != 0 { "open".to_owned() } else { "closed".to_owned() }),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = 218 + index as i32 * 18;
        small(image, font, label, MUTED, 28, y);
        small(image, font, value, TEXT, 170, y);
    }
    if view.leader() && info.members > 1 {
        small(image, font, "Pass the leadership before leaving.", MUTED, 172, 350);
    }
}

fn draw_members(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let members = &view.list(GuildPanelPacket::MEMBERS).rows;
    for (x, label) in [(28, "Name"), (190, "Class"), (290, "Level"), (340, "Rank"), (430, "Status")] {
        small(image, font, label, GOLD, x, 132);
    }
    let start = view.member_page as usize * MEMBER_ROWS;
    for (index, member) in members.iter().skip(start).take(MEMBER_ROWS).enumerate() {
        let rect = UiRect::new(20, 150 + index as i32 * 22, 480, 21);
        let selected = view.selected == Some(member.id);
        draw_card(image, rect, if selected { WidgetState::Selected } else { WidgetState::Normal });
        let colour = if member.value >= LEADER { GOLD } else { TEXT };
        small(image, font, &clip(&member.text1, 22), colour, 28, rect.y + 4);
        small(image, font, &member.text2, MUTED, 190, rect.y + 4);
        small(image, font, &member.level.to_string(), TEXT, 290, rect.y + 4);
        small(image, font, rank_name(member.value), colour, 340, rect.y + 4);
        let (status, colour) = if member.extra == 1 { ("Online", GREEN) } else { ("Offline", MUTED) };
        small(image, font, status, colour, 430, rect.y + 4);
    }
    let pages = (members.len().div_ceil(MEMBER_ROWS) as u32).max(1);
    page_label(image, font, 310, view.member_page, pages);
    if view.selected.is_none() {
        small(image, font, "Click a member for the actions.", MUTED, 24, 350);
    }
}

fn draw_requests(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    if view.officer() {
        draw_field(image, font, view, Field::InviteName, "Name of the player to invite");
    } else {
        small(image, font, "Only the leader and the deputies invite and accept.", MUTED, 24, 136);
    }
    let invites = &view.list(GuildPanelPacket::INVITES).rows;
    say(image, font, &format!("Invitations sent ({}) - they expire in 7 days", invites.len()), GOLD, 24, 158);
    if invites.is_empty() {
        small(image, font, "No pending invitations.", MUTED, 24, 178);
    }
    for (index, invite) in invites.iter().take(INVITE_ROWS).enumerate() {
        let y = 176 + index as i32 * 19;
        small(image, font, &format!("{} (Lv {}) - by {}", invite.text1, invite.level, invite.text2), TEXT, 28, y + 2);
        small(image, font, &ago(invite.at, view.now), MUTED, 300, y + 2);
    }
    let requests = &view.list(GuildPanelPacket::REQUESTS).rows;
    say(image, font, &format!("Pending requests to join ({})", requests.len()), GOLD, 24, 236);
    if requests.is_empty() {
        small(image, font, "No requests.", MUTED, 24, 256);
    }
    let start = view.request_page as usize * REQUEST_CARDS;
    for (index, request) in requests.iter().skip(start).take(REQUEST_CARDS).enumerate() {
        let y = 254 + index as i32 * 46;
        draw_card(image, UiRect::new(20, y, 480, 43), WidgetState::Normal);
        let (status, colour) = if request.extra == 1 { ("Online", GREEN) } else { ("Offline", MUTED) };
        labels::text_sized(
            image,
            font,
            &[
                (request.text1.as_str(), GOLD),
                (&format!("  {} - Level {} - ", request.text2, request.level), MUTED),
                (status, colour),
            ],
            [28.0, y as f32 + 4.0],
            11.0,
        );
        small(image, font, &when(view, request.at), MUTED, 360, y + 4);
        small(image, font, &clip(&request.text3, 46), TEXT, 28, y + 24);
    }
    let pages = (requests.len().div_ceil(REQUEST_CARDS) as u32).max(1);
    if requests.len() > REQUEST_CARDS {
        page_label(image, font, 348, view.request_page, pages);
    }
}

fn draw_history(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    for (index, label) in ["All", "Members", "Bank", "War", "Settings"].into_iter().enumerate() {
        let rect = UiRect::new(24 + index as i32 * 95, 128, 91, 20);
        draw_tab(image, font, rect, label, view.history_filter as usize == index, hovered(view, GuildAction::HistoryFilter(index as u8)));
    }
    draw_field(image, font, view, Field::HistorySearch, "Search by player name...");
    let events = view.list(GuildPanelPacket::EVENTS);
    if events.rows.is_empty() {
        small(image, font, "No events.", MUTED, 24, 186);
    }
    for (index, event) in events.rows.iter().take(PAGE_ROWS).enumerate() {
        let y = 182 + index as i32 * 17;
        let colour = event_colour(&event.text1);
        fill(image, UiRect::new(26, y + 5, 6, 6), [colour[0], colour[1], colour[2], 255]);
        small(image, font, &clip(&event_text(event), 52), TEXT, 38, y + 1);
        small(image, font, &when(view, event.at), MUTED, 346, y + 1);
    }
    let page = view.pages[GuildPanelPacket::EVENTS as usize];
    page_label(image, font, 330, page, events.pages());
    small(image, font, &format!("{} events - kept for 90 days", events.total()), MUTED, 24, 352);
}

fn draw_balance(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let info = view.info.clone().unwrap_or_default();
    draw_card(image, UiRect::new(20, 128, 480, 50), WidgetState::Normal);
    labels::text_sized(image, font, &[(format!("{} silver in the guild fund", thousands(info.fund)).as_str(), GOLD)], [28.0, 131.0], 13.0);
    let fee = if info.entry_fee > 0 { format!("{} silver", thousands(u64::from(info.entry_fee))) } else { "none".to_owned() };
    small(image, font, &format!("Entry fee: {fee} (the leader sets it in Settings); it goes to the fund."), TEXT, 28, 148);
    small(image, font, &format!("Your silver: {}. Deposit: everyone. Withdraw: the leader only.", thousands(info.silver)), MUTED, 28, 162);
    draw_field(image, font, view, Field::Amount, "Amount in silver");
    let balance = view.list(GuildPanelPacket::BALANCE);
    for (x, label) in [(28, "Type"), (120, "Player"), (250, "Amount"), (330, "Balance after"), (420, "Date")] {
        small(image, font, label, GOLD, x, 214);
    }
    if balance.rows.is_empty() {
        small(image, font, "No movements yet.", MUTED, 28, 232);
    }
    for (index, movement) in balance.rows.iter().take(PAGE_ROWS).enumerate() {
        let y = 230 + index as i32 * 13;
        let kind = match movement.text1.as_str() {
            "deposit" => ("Deposit", GREEN),
            "withdraw" => ("Withdrawal", RED),
            _ => ("Fee", GOLD),
        };
        small(image, font, kind.0, kind.1, 28, y);
        small(image, font, &clip(&movement.text2, 16), TEXT, 120, y);
        small(image, font, &thousands(u64::from(movement.value)), TEXT, 250, y);
        small(image, font, &thousands(u64::from(movement.extra)), MUTED, 330, y);
        small(image, font, &date(movement.at)[..10], MUTED, 420, y);
    }
    page_label(image, font, 340, view.pages[GuildPanelPacket::BALANCE as usize], balance.pages());
}

fn war_state(value: u32) -> (&'static str, [u8; 3]) {
    match value {
        0 => ("Declared on us", [230, 160, 90]),
        1 => ("Awaiting an answer", GOLD),
        2 => ("At war", RED),
        3 => ("Refused", MUTED),
        _ => ("Ended", MUTED),
    }
}

fn draw_war(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let wars = &view.list(GuildPanelPacket::WARS).rows;
    draw_card(image, UiRect::new(16, 128, 208, 210), WidgetState::Normal);
    say(image, font, "Wars", GOLD, 24, 131);
    if wars.is_empty() {
        small(image, font, "No wars yet.", MUTED, 24, 154);
    }
    for (index, war) in wars.iter().take(8).enumerate() {
        let rect = UiRect::new(20, 150 + index as i32 * 23, 200, 22);
        let selected = view.selected_war == Some(war.id);
        draw_card(image, rect, if selected { WidgetState::Selected } else { WidgetState::Normal });
        small(image, font, &clip(&war.text1, 16), TEXT, rect.x + 6, rect.y + 4);
        let (_, colour) = war_state(war.value);
        small(image, font, &format!("{} x {}", war.level, war.extra), colour, rect.x + 150, rect.y + 4);
    }
    if view.leader() {
        draw_field(image, font, view, Field::WarName, "Target guild");
    }
    draw_card(image, UiRect::new(230, 128, 274, 240), WidgetState::Normal);
    let Some(war) = view.selected_war.and_then(|id| wars.iter().find(|row| row.id == id)) else {
        say(image, font, "War details", GOLD, 238, 131);
        let help = [
            "Pick a war in the list to see its score.",
            "To declare war, type the target guild's",
            "name and click Declare. It may accept",
            "or refuse. At war, every death between",
            "the two guilds counts in the score.",
        ];
        for (index, line) in help.iter().enumerate() {
            small(image, font, line, TEXT, 238, 154 + index as i32 * 16);
        }
        return;
    };
    say(image, font, &format!("Against {}", clip(&war.text1, 20)), GOLD, 238, 131);
    let (state, colour) = war_state(war.value);
    small(image, font, &format!("{state} - since {}", date(war.at)), colour, 238, 150);
    small(image, font, &format!("Score: us {} x {} them", war.level, war.extra), TEXT, 238, 166);
    let kills = view.list(GuildPanelPacket::WAR_KILLS);
    for (x, label) in [(238, "Killer"), (350, "Victim"), (450, "Date")] {
        small(image, font, label, GOLD, x, 210);
    }
    if kills.rows.is_empty() {
        small(image, font, "No deaths yet.", MUTED, 238, 228);
    }
    for (index, kill) in kills.rows.iter().take(PAGE_ROWS).enumerate() {
        let y = 226 + index as i32 * 13;
        let colour = if kill.value == 1 { GREEN } else { RED };
        small(image, font, &clip(&kill.text1, 14), colour, 238, y);
        small(image, font, &clip(&kill.text2, 13), TEXT, 350, y);
        small(image, font, &date(kill.at)[..5], MUTED, 450, y);
    }
    small(image, font, &format!("{} deaths", kills.total()), MUTED, 318, 347);
}

fn draw_deaths(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    for (index, label) in ["All", "Deaths", "Frags"].into_iter().enumerate() {
        let rect = UiRect::new(24 + index as i32 * 76, 128, 72, 20);
        draw_tab(image, font, rect, label, view.death_kind as usize == index, hovered(view, GuildAction::DeathKind(index as u8)));
    }
    for (index, label) in ["Any cause", "Players", "Monsters"].into_iter().enumerate() {
        let rect = UiRect::new(268 + index as i32 * 76, 128, 72, 20);
        draw_tab(image, font, rect, label, view.death_cause as usize == index, hovered(view, GuildAction::DeathCause(index as u8)));
    }
    draw_field(image, font, view, Field::DeathSearch, "Search by player name...");
    let deaths = view.list(GuildPanelPacket::DEATHS);
    for (x, label) in [(28, "Date and time"), (150, "Victim"), (268, "Level"), (320, "Killer (guild)")] {
        small(image, font, label, GOLD, x, 184);
    }
    if deaths.rows.is_empty() {
        small(image, font, "Nothing found for this filter.", MUTED, 28, 204);
    }
    for (index, death) in deaths.rows.iter().take(PAGE_ROWS).enumerate() {
        let y = 200 + index as i32 * 15;
        let colour = if death.value == 1 { GREEN } else { RED };
        small(image, font, &date(death.at), MUTED, 28, y);
        small(image, font, &clip(&death.text1, 15), colour, 150, y);
        small(image, font, &death.level.to_string(), TEXT, 268, y);
        let killer = if death.text3.is_empty() { death.text2.clone() } else { format!("{} ({})", death.text2, death.text3) };
        small(image, font, &clip(&killer, 26), TEXT, 320, y);
    }
    let header = &deaths.header;
    small(
        image,
        font,
        &format!("Summary: 12H {} | 24H {} | 3D {} | 7D {} records", header.level, header.value, header.extra, header.id),
        MUTED,
        24,
        324,
    );
    page_label(image, font, 344, view.pages[GuildPanelPacket::DEATHS as usize], deaths.pages());
}

fn draw_settings(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    let Some(info) = &view.info else {
        return;
    };
    if !view.leader() {
        say(image, font, "Guild settings", GOLD, 24, 132);
        small(image, font, "Only the leader changes the title, the motto, the description,", TEXT, 24, 156);
        small(image, font, "the entry fee and whether the guild takes requests.", TEXT, 24, 172);
        small(image, font, &format!("Entry fee: {} silver", thousands(u64::from(info.entry_fee))), MUTED, 24, 200);
        small(image, font, if info.accepting != 0 { "Taking requests." } else { "Not taking requests." }, MUTED, 24, 216);
        return;
    }
    let labels_at = [
        (Field::SetTitle, "Title", "A short subtitle"),
        (Field::SetMotto, "Motto", "A short phrase"),
        (Field::SetDescription, "Description", "The guild's presentation"),
        (Field::SetFee, "Entry fee (silver)", "0"),
    ];
    for (field, label, placeholder) in labels_at {
        let rect = field_rect(field);
        small(image, font, label, TEXT, rect.x, rect.y - 14);
        draw_field(image, font, view, field, placeholder);
    }
    small(image, font, "Disbanding erases the guild,", MUTED, 192, 344);
    small(image, font, "its fund and its history.", MUTED, 192, 357);
}

fn draw_crest_picker(image: &mut RgbaImage, font: &Font, view: &GuildView) {
    use crate::guild_emblems::{colour_of, draw_crest, field_of, icon_of, EMBLEMS, EMBLEM_COLOURS, FIELD_COLOURS};
    let top = crest_top(view);
    let draft = view.crest_draft;
    draw_crest(image, draft, 22, top, 80, 90);
    centered_text(image, font, EMBLEMS[icon_of(draft)].0, 62.0, top as f32 + 96.0, 11.0, GOLD);
    if view.in_guild() && !view.leader() {
        small(image, font, "Only the leader", MUTED, 24, top + 116);
        small(image, font, "changes the crest.", MUTED, 24, top + 130);
    }
    for index in 0..EMBLEMS.len() {
        let cell = emblem_cell(view, index);
        let chosen = icon_of(draft) == index;
        let hovered = view.hover == Some(GuildAction::PickEmblem(index as u8));
        fill(image, cell, if chosen { [80, 57, 29, 255] } else if hovered { [24, 48, 62, 255] } else { [9, 24, 34, 230] });
        outline(image, cell, if chosen || hovered { ui_theme::PANEL_HIGHLIGHT } else { [55, 72, 82, 220] });
        let crest = crate::guild_emblems::compose(index, field_of(draft), colour_of(draft));
        draw_crest(image, crest, cell.x + 6, cell.y + 3, 28, 32);
    }
    small(image, font, "Field", TEXT, 24, crest_top(view) + 169);
    for (index, (light, dark)) in FIELD_COLOURS.iter().enumerate() {
        let swatch = field_swatch(view, index);
        swatch_box(image, swatch, *light, *dark, field_of(draft) == index);
    }
    small(image, font, "Emblem", TEXT, 24, crest_top(view) + 195);
    for (index, (light, dark)) in EMBLEM_COLOURS.iter().enumerate() {
        let swatch = colour_swatch(view, index);
        swatch_box(image, swatch, *light, *dark, colour_of(draft) == index);
    }
}

/// A colour to pick: its light over its dark, gold-framed when chosen.
fn swatch_box(image: &mut RgbaImage, rect: UiRect, light: [u8; 3], dark: [u8; 3], chosen: bool) {
    fill(image, UiRect::new(rect.x, rect.y, rect.width, rect.height / 2), [light[0], light[1], light[2], 255]);
    fill(image, UiRect::new(rect.x, rect.y + rect.height / 2, rect.width, rect.height - rect.height / 2), [dark[0], dark[1], dark[2], 255]);
    outline(image, rect, if chosen { ui_theme::PANEL_HIGHLIGHT } else { [20, 20, 20, 255] });
    if chosen {
        outline(image, UiRect::new(rect.x - 2, rect.y - 2, rect.width + 4, rect.height + 4), ui_theme::PANEL_HIGHLIGHT);
    }
}

/// Draws the window onto the quick panel's contents.
pub fn draw(image: &mut RgbaImage, font: &Font, view: Option<&GuildView>) {
    let Some(view) = view.filter(|view| view.info.is_some()) else {
        say(image, font, "Guild System", GOLD, 24, 40);
        small(image, font, "Waiting for the server...", MUTED, 24, 62);
        return;
    };
    draw_header(image, font, view);
    match view.tab {
        Tab::System => draw_system(image, font, view),
        Tab::Search => draw_search(image, font, view),
        Tab::Create => draw_create(image, font, view),
        Tab::Overview => draw_overview(image, font, view),
        Tab::Members => draw_members(image, font, view),
        Tab::Requests => draw_requests(image, font, view),
        Tab::History => draw_history(image, font, view),
        Tab::Balance => draw_balance(image, font, view),
        Tab::War => draw_war(image, font, view),
        Tab::Deaths => draw_deaths(image, font, view),
        Tab::Settings => draw_settings(image, font, view),
        Tab::Crest => draw_crest_picker(image, font, view),
    }
    draw_buttons(image, font, view);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gravak_protocol::GuildPanelPacket as P;

    fn row(list: u32, id: u32, texts: [&str; 4], numbers: [u32; 3], at: u64) -> GuildPanelRow {
        GuildPanelRow {
            list,
            index: 1,
            id,
            level: numbers[0],
            value: numbers[1],
            extra: numbers[2],
            at,
            text1: texts[0].into(),
            text2: texts[1].into(),
            text3: texts[2].into(),
            text4: texts[3].into(),
            ..Default::default()
        }
    }

    fn header(list: u32, total: u32) -> GuildPanelRow {
        GuildPanelRow { list, total, ..Default::default() }
    }

    const NOW: u64 = 1_791_400_000;

    fn member_view() -> GuildView {
        let mut view = GuildView { now: NOW / 60, hero: "Nattank".into(), ..Default::default() };
        view.receive_info(GuildPanelInfo {
            in_guild: 1,
            guild_id: 7,
            role: 1_000,
            level: 3,
            experience: 120_000,
            next_experience: 700_000,
            members: 4,
            max_members: 50,
            online: 2,
            fund: 1_245_000,
            entry_fee: 5_000,
            accepting: 1,
            created_at: NOW - 86_400 * 3,
            guilds: 2,
            guilds_accepting: 2,
            silver: 3_400_000,
            hero_level: 120,
            pending_requests: 3,
            emblem: 1 | 3 << 8,
            name: "Yellow Wall".into(),
            title: "Reis Veteranos".into(),
            motto: "Reis Vitoriosos".into(),
            description: "A guild mais antiga de Twin City. Caçamos juntos, dividimos o cofre e defendemos os nossos em guerra.".into(),
            leader: "Nattank".into(),
            note: "Menozin joined the guild.".into(),
        });
        let lists: Vec<(u32, Vec<GuildPanelRow>)> = vec![
            (P::MEMBERS, vec![
                row(P::MEMBERS, 1, ["Nattank", "Trojan", "", ""], [130, 1_000, 1], NOW),
                row(P::MEMBERS, 2, ["Menozin Is Back", "Warrior", "", ""], [124, 990, 1], NOW),
                row(P::MEMBERS, 3, ["Greg Jr", "Archer", "", ""], [88, 200, 0], NOW),
                row(P::MEMBERS, 4, ["Jhow", "Fire Taoist", "", ""], [70, 200, 0], NOW),
            ]),
            (P::REQUESTS, vec![
                row(P::REQUESTS, 5, ["Greg Jr On Orion", "Archer", "REI, ME ACEITA", ""], [8, 0, 1], NOW - 120),
                row(P::REQUESTS, 6, ["Jhow On Orion", "Trojan", "SALVE REI, QUERO ENTRAR NA TUA GUILD", ""], [8, 0, 0], NOW - 4_000),
                row(P::REQUESTS, 7, ["Third One", "Warrior", "oi", ""], [50, 0, 0], NOW - 90_000),
            ]),
            (P::INVITES, vec![row(P::INVITES, 8, ["Fulano", "Nattank", "", ""], [77, 0, 0], NOW - 600)]),
            (P::EVENTS, vec![
                row(P::EVENTS, 1, ["founded", "Nattank", "", ""], [0, 0, 0], NOW - 86_400 * 3),
                row(P::EVENTS, 2, ["joined", "Menozin Is Back", "Nattank", ""], [0, 0, 0], NOW - 3_600),
                row(P::EVENTS, 3, ["deposit", "Menozin Is Back", "", ""], [0, 50_000, 1_245_000], NOW - 1_800),
                row(P::EVENTS, 4, ["war_started", "Yellow Wall", "Red Tide", ""], [0, 0, 0], NOW - 60),
            ]),
            (P::BALANCE, vec![
                row(P::BALANCE, 1, ["deposit", "Menozin Is Back", "", ""], [0, 50_000, 1_245_000], NOW - 1_800),
                row(P::BALANCE, 2, ["fee", "Greg Jr", "", ""], [0, 5_000, 1_195_000], NOW - 3_600),
                row(P::BALANCE, 3, ["withdraw", "Nattank", "", ""], [0, 10_000, 1_190_000], NOW - 7_200),
            ]),
            (P::WARS, vec![
                row(P::WARS, 11, ["Red Tide", "", "", ""], [3, 2, 1], NOW - 86_400),
                row(P::WARS, 12, ["Blue Moon", "", "", ""], [0, 0, 0], NOW - 600),
            ]),
            (P::WAR_KILLS, vec![
                row(P::WAR_KILLS, 1, ["Menozin Is Back", "RivalOne", "", ""], [0, 1, 0], NOW - 600),
                row(P::WAR_KILLS, 2, ["RivalTwo", "Jhow", "", ""], [0, 0, 0], NOW - 300),
            ]),
            (P::DEATHS, vec![
                row(P::DEATHS, 1, ["Jhow", "FiendBat", "", ""], [70, 0, 0], NOW - 900),
                row(P::DEATHS, 2, ["RivalOne", "Menozin Is Back", "Yellow Wall", ""], [118, 1, 1], NOW - 600),
            ]),
        ];
        for (list, rows) in lists {
            view.receive_row(header(list, rows.len() as u32));
            for line in rows {
                view.receive_row(line);
            }
        }
        view.selected_war = Some(11);
        view
    }

    fn outside_view() -> GuildView {
        let mut view = GuildView { now: NOW / 60, ..Default::default() };
        view.receive_info(GuildPanelInfo {
            guilds: 2,
            guilds_accepting: 2,
            silver: 1_245_000,
            hero_level: 95,
            ..Default::default()
        });
        view.receive_row(header(P::SEARCH, 2));
        view.receive_row(row(P::SEARCH, 7, ["Yellow Wall", "Reis Veteranos", "Nattank", "Reis Vitoriosos"], [3, 4, 1], 0));
        view.receive_row(row(P::SEARCH, 8, ["Red Tide", "", "RivalLeader", ""], [0, 1, 2], 18 | 4 << 8 | 4 << 16));
        view.receive_row(header(P::MY_INVITES, 1));
        view.receive_row(row(P::MY_INVITES, 7, ["Yellow Wall", "Nattank", "", ""], [0, 0, 0], NOW - 300));
        view.receive_row(header(P::MY_REQUESTS, 1));
        view.receive_row(row(P::MY_REQUESTS, 8, ["Red Tide", "quero entrar", "", ""], [0, 0, 0], NOW - 3_000));
        view
    }

    #[test]
    fn the_window_follows_whether_there_is_a_guild() {
        let view = member_view();
        assert_eq!(view.tab, Tab::Overview);
        assert_eq!(view.input(Field::SetTitle), "Reis Veteranos", "the settings fill once");
        assert_eq!(view.list(P::MEMBERS).rows.len(), 4);
        let outside = outside_view();
        assert_eq!(outside.tab, Tab::System);
    }

    #[test]
    fn clicks_become_the_servers_requests() {
        let mut view = member_view();
        view.tab = Tab::Members;
        // A member picked, then promoted.
        assert_eq!(action_at(&view, [100.0, 196.0]), Some(GuildAction::Select(3)));
        view.apply(GuildAction::Select(3));
        let promote = action_at(&view, [60.0, 356.0]).expect("Promover");
        let packet = view.apply(promote).expect("a request");
        assert_eq!((packet.action, packet.param), (P::PROMOTE, 3));
        // Disbanding asks twice.
        assert!(view.apply(GuildAction::Disband).is_none());
        assert_eq!(view.apply(GuildAction::Disband).map(|packet| packet.action), Some(P::DISBAND));
        // Typing an amount and depositing it.
        view.apply(GuildAction::Focus(Field::Amount));
        view.type_text("12a3");
        assert_eq!(view.input(Field::Amount), "123");
        let deposit = view.apply(GuildAction::Deposit).expect("a deposit");
        assert_eq!((deposit.action, deposit.amount), (P::DEPOSIT, 123));
        // The history filter asks its page.
        let page = view.apply(GuildAction::HistoryFilter(2)).expect("a page");
        assert_eq!((page.action, page.param, page.text1.as_str()), (P::PAGE, P::EVENTS, "fund"));
    }

    #[test]
    fn dates_read_in_the_players_time() {
        assert_eq!(date(0), "31/12/1969 21:00");
        assert_eq!(ago(NOW - 30, NOW / 60), "just now");
        assert_eq!(ago(NOW - 7_200, NOW / 60), "2 h ago");
    }

    /// `GUILD_PREVIEW=dir cargo test -p conquer-sandbox guild_preview`.
    #[test]
    fn guild_preview() {
        let Ok(dir) = std::env::var("GUILD_PREVIEW") else { return };
        let font = crate::labels::font().expect("font");
        let tabs = [
            ("outside-system", Tab::System, false),
            ("outside-search", Tab::Search, false),
            ("outside-create", Tab::Create, false),
            ("overview", Tab::Overview, true),
            ("members", Tab::Members, true),
            ("requests", Tab::Requests, true),
            ("history", Tab::History, true),
            ("balance", Tab::Balance, true),
            ("war", Tab::War, true),
            ("deaths", Tab::Deaths, true),
            ("settings", Tab::Settings, true),
            ("crest", Tab::Crest, true),
            ("outside-crest", Tab::Crest, false),
        ];
        for (name, tab, inside) in tabs {
            let mut view = if inside { member_view() } else { outside_view() };
            view.tab = tab;
            if tab == Tab::Members {
                view.selected = Some(3);
            }
            if tab == Tab::Crest {
                view.crest_return = if inside { Tab::Settings } else { Tab::Create };
                view.crest_draft = crate::guild_emblems::compose(5, 1, 0);
            }
            if tab == Tab::Search {
                view.selected = Some(7);
            }
            if tab == Tab::Create {
                view.inputs[Field::CreateName.index()] = "Yellow Wall".into();
                view.focused = Some(Field::CreateTitle);
                view.inputs[Field::CreateTitle.index()] = "Reis Vete".into();
            }
            let mut content = image::RgbaImage::new(520, 420);
            draw(&mut content, &font, Some(&view));
            let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
            let image = crate::shop::framed(&mut assets, content, "Guild System", false).expect("frame");
            image.save(format!("{dir}/guild-{name}.png")).expect("png");
        }
    }
}
