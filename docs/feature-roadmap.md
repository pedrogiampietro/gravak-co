# Gravak Feature Roadmap

This roadmap compares the current Rust implementation with the behavior found
in `R:\ReduxConquerProject`. The legacy project is a protocol and gameplay
reference, not an architecture to copy.

## Current State

The Gravak vertical slice already covers authentication, character loading,
maps, collision, portals, movement, NPC spawns, NPC dialogs, NPC shops,
inventory, warehouse storage, chat UI, the 5065 client renderer, and the first
server-authoritative PvE combat loop. The first data-driven quest now runs from
the OldGeneralYang dialog through monster progress, persistent completion and
an idempotent reward; `$quest-status` exposes the saved state.

The client and server are still crossing a 5065/5017 compatibility boundary.
Packet layouts and semantics must be verified before adding large gameplay
systems.

## Priority Gaps

### P0: Client performance and UI/UX

- [x] The client now reuses a dynamic GPU vertex buffer instead of creating one
  buffer per draw on every frame; the focused checklist is
  `docs/client-ui-performance-checklist.md`. Release measurements are ~0.7
  ms/frame offline and ~0.77–0.81 ms/frame in the final online 50-bot sample,
  with worst frames of 4.16 ms and 1.91 ms after remote actor pose/label
  preparation was moved off the critical path and budgeted per frame.
- [x] The initial sync no longer starts a false level-up effect, and the static
  index for the 9,376-sprite map is primed before the first interactive frame.
  The latest authenticated online Release/immediate benchmark measured
  ~0.83–0.84 ms/frame, with 1.92–3.72 ms peaks, 85–87 draws and
  21,648–21,660 vertices.
- [x] Equipment updates no longer decode the local paper-doll synchronously on
  the window thread. Login/equipment bursts now use the dedicated model worker;
  the latest authenticated Release/immediate cycles stayed around ~1.31–1.42
  ms/frame, with only an isolated ~12.4 ms spike during the longer sample.
- [x] Add optional FPS/frame-time/draw-call/texture diagnostics and establish a
  60–500 FPS budget in Release mode; `GRAVAK_PERF=1` and `F11` expose the
  metrics, normal play follows monitor VSync, fixed caps are available through
  `GRAVAK_MAX_FPS=60|100|200|300|400|500`, and immediate presentation remains a
  benchmark-only option.
- [x] Replace full-map visibility rescans on camera-cell changes with a spatial
  index for static sprites; the Release/immediate 500-FPS smoke stayed around
  0.57–0.75 ms/frame with no repeated 20+ ms walking hitch.
- [~] Refactor the renderer into invalidated world, actor/effects and UI phases;
  static map metadata, camera-cell visibility caching, compatible adjacent
  draws, culling, async texture preparation and texture-load budgeting are
  implemented, while full phase invalidation and static vertex reuse remain.
- [~] Establish shared responsive UI components and tokens, then refactor
  Character and Inventory before Shop, Warehouse, Trade, Guild and dialogs.

### P0: Protocol contract

- Add fixtures for encode/decode of the 5065 packets used by the client. The current suite covers valid and truncated payloads for the priority packets; poker `1012` and full handler-level rejection coverage remain.
- Verify action, NPC, item, warehouse, talk, and map packet IDs and payloads.
- Keep wire structs separate from domain models.
- Add validation for timestamps, character identity, map identity, and ownership.

### P1: First playable combat loop

Status: core do loop já está implementado no servidor; permanecem a integração
de feedback no cliente e as regras avançadas de combate. O PvE agora também
considera a vantagem de nível contra monstros, com teto seguro e sem aplicar
esse bônus ao PvP.

- [x] Add a persistent monster definition and map spawn model.
- [x] Add a `MsgInteract` 1022 wire packet for physical attacks.
- [x] Validate target visibility, map, distance, and alive state.
- [x] Resolve physical damage on the server and broadcast the result.
- [x] Add monster death, XP, respawn, and ground drops.
- [~] Add client combat feedback and cooldowns: physical hit/damage numbers, target animation, weapon cadence, durability, lethal action 14, ground drops and 1101 pickup are wired and covered by `gravak-probe --combat`; PvE physical/magic damage now includes a capped level advantage and the Archer family physical multiplier, while character targets resolve behind the PK mode, and `gravak-probe --pvp` confirms two-session physical/magic damage, `Safe` blocking, death blocking and `Reborn` HP restoration; advanced combat feedback/rules remain.

### P2: Progression

- [x] Make NPC skill training catalog-driven: the trainer selects the next unlearned skill eligible by level and silver cost with deterministic ordering and atomic charging.
- [x] Add `gravak-probe --npc-training`/`--npc-training-positive` coverage for the dialog option, no-duplicate eligibility path and positive `1103` learning; `$test-training` prepares an eligible local fixture and the smoke confirms the 500-silver charge.

- Equip and unequip items.
- [x] Consume direct HP/MP potions and atomic consumable packs.
- [x] Persist and synchronize the first temporary effect: item `723017` grants double XP for 60 minutes.
- [x] Persist and synchronize Heaven's Blessing (`1200000..1200002`), granting +30% experience for 3/7/30 days.
- [~] Add the first playable PvE skills: Basic Fire (`1000`), Flame Burst (`1001`), Whirlwind (`1002`), the self-buff Cyclone (`1110`), the self-buff Superman (`1025`), the self-buff Stigma (`1095`) and Magic Shield (`1090`) with server-side target/range/cooldown/mana/damage validation, F1/F2/F3 action-bar triggers, multi-target 1105 feedback and catalog-driven statuses; costs/range/cooldown/radius/power/status/duration load from `server/game/data/skills.dat`, learned skills persist in `character_skills`, Cyclone applies the classic `1<<23` flag for 10 seconds and reduces the physical attack interval to one third, Superman applies `1<<18` for 15 seconds and multiplies physical damage by 10 in PvE/2 in PvP, Stigma applies `1<<9` for 70 seconds and adds 20% attack, Magic Shield applies `1<<8` for 70 seconds and adds 20% defense, level/proficiency now scale skill power with a safe cap, equipped `MagicAttack`/`MagicDefense` values from `itemtype.dat` feed the magical path, Flame Burst applies Burning and Whirlwind applies a 1.5-second Stunned that blocks movement/attack/skill; broader spell content remains.
- [x] Add weapon durability loss on attacks and atomic repair through blacksmith NPCs; broken equipment is excluded from combat/resource stats.
- [x] Add base attributes and points: level-up grants points, packet `1024 AssignAttributes` spends them atomically and persists the result, client `+` buttons send the classic wire shape, and `MsgUserInfo` confirms the update; remaining-points feedback can still be refined.
- [~] Add item enhancement progression: `item_upgrades` persists `+0..+12` and two sockets by UID, the client context menu sends `1009/19` quality and `1009/20` level upgrades, ArtisanWind offers both services, `1009/43` opens sockets with DragonBalls, `1027` inserts/removes gem codes atomically, and Jewelers compose 15 equal gems into the next tier with silver. `MsgItemInfo` sends classic `0/255/code` socket values and `--sockets` covers the end-to-end cycle. Dragon/Tortoise physical combat bonuses and quality transitions are authoritative. Refinery packet `2076`, seven-day persistence, slot compatibility, `REFINERY` context action, Intensification and Penetration are implemented. Artifact action `2076/1`, seven-day persistence, slot compatibility, `ARTIFACT` context action, atomic artifact+Meteor consumption, fire-resistance derivation for active equipment and `2077` synchronization are also implemented. Character-targeted magic damage now applies the active fire resistance; remaining elements and other gem/refinery effects remain.
- [~] Load weapon proficiencies and spells: weapon families now persist level/experience in `character_weapon_proficiencies`, gain practice from physical attacks using the classic 5065 thresholds through level 20, and add a capped bonus for the active durable weapon; `itemtype.dat` profession, weapon-skill, level, sex and base-attribute requirements are parsed and enforced before equipment; classic family rules and spell-tree content remain.
- [x] Apply the classic Trojan life progression: profession levels `0..5` now use the reference `100/105/108/110/112/115%` multiplier for attribute-derived life and for new attribute allocations; other class families keep their current formulas.
- [x] Apply the classic Taoist mana progression: Water/Fire Taoist profession levels `0..5` now use the reference `100/100/300/400/500/600%` multiplier over the base `5 * Spirit` capacity.
- [~] Add skills, cooldowns, mana, buffs, and debuffs: Basic Fire, Flame Burst, Whirlwind, Cyclone, Superman, Stigma and Magic Shield use external data including power/status/duration, learned skills persist and synchronize through 1103, skill books consume atomically, NPC trainers validate proximity, level and silver cost, and the client assigns learned skills to the action bar; skill and first weapon-proficiency scaling are in place, Cyclone consumes mana, applies an expiring classic speed buff and reduces physical cadence to one third, Superman consumes mana, applies an expiring classic damage buff and multiplies physical damage by 10 in PvE/2 in PvP, Stigma consumes mana and applies an expiring 20% attack buff, Magic Shield consumes mana and applies an expiring 20% defense buff, Flame Burst applies an expiring Burning defense debuff plus 5% max-health damage per second with server-controlled ticks, and Whirlwind applies an expiring Stunned that blocks movement/attack/skill; statuses reach `MsgPlayer`, show `BURN`/`STUN`/`CYCL`/`SUPER`/`STIG`/`SHLD` in the HUD, and are covered by the corresponding probe smokes; class requirements, additional buffs/debuffs and status content remain.

### P3: World content

- [~] Build a data-driven NPC and quest system: `quest_definitions` and `character_quests` now power the first `Pheasant Hunt` quest, accepted through `OldGeneralYang`, progressed by a monster defeat, queried through `$quest-status` and completed with an idempotent silver/XP reward; the five city Conductresses (Twin City, Desert City, Phoenix Castle, Ape Mountain, Bird Island) now teleport to their own city gates and the Market for 100 silver, resolved on the server from `teleports.rs`, with the destination cell validated before the fee is charged and a refund if the map change fails; Mark.Controller returns players to where they entered the Market, and the prison Wardens no longer offer a way out; quest chains, requirements, additional objectives and the remaining service NPCs remain.
- [~] Add monster AI, patrols, aggro, respawn and data-driven drops: monster definitions/spawns are persisted and migration `39_monster_catalog.sql` now seeds the reference catalog (405 types, 1,005 spawns, ~8,400 monsters across Twin City, Desert City, Phoenix Castle, Ape Mountain, Bird Island, Mystic Castle and the canyon maps; city guards are left out until PK rules exist), runtime respawn/AI is active with monsters kept inside their spawn area, relocated off blocked cells and idle on maps without players, and item drops load from `monster_drops` with per-monster chance, validated min/max quantities, monster-level ranges and exclusive groups; patrol policies and special drops remain.
- Add teleport NPCs, city services, events, and scripted interactions.

### P4: Social and operations

- [~] Complete the first social slice: channel `2001` routes `/w` and
  `/whisper` by character name, echoes to the sender and rejects unknown
  recipients; packet `1019` persists friend request/accept/remove and reloads
  the list on login. Ignore now uses actions `19/18`, persists blocked
  characters, reloads them on login and blocks whisper delivery. The client
  now keeps a filtered, five-line session chat history with Clear and renders
  the first Friends panel with friend/ignore online state. The panel now wires
  add/remove friend, add/remove ignore, and whisper preparation through the
  server-authoritative `1019` actions. Whisper and regional `Talk`/`World` history
  now persist per character with a 40-message retention and replay on login;
  the client now renders an in-world status banner for confirmations/errors;
  contextual social feedback remains; advanced guild roles/war and faction
  systems remain.
- [~] Add the first team vertical slice: classic `1023` actions and `1026`
  member synchronization now support create, invite/accept, join requests,
  leave, kick, dismissal, leadership transfer, a five-member cap and disconnect cleanup. Nearby
  living members now split monster XP and can pick up the party-owned drop;
  the client consumes the packets and renders the Team panel with member names,
  leader marker, colored HP bars and current/max values; the panel now sends create, accept-invite, request/accept-join,
  invite, leave, leader, kick, dismiss and join-policy actions, using selected
  world/member targets.
  Advanced loot rules and a complete graphical smoke remain.
- [~] Add secure player trade on classic `1056`: request/accept, item and silver
  offers, cancellation, double confirmation and an atomic server-side exchange
  that preserves UIDs/upgrades and revalidates ownership, capacity and balance.
  The two-session `--trade` probe and the client's Trade panel, including
  inventory drag-and-drop, are wired; the server expires abandoned sessions
  after 60 seconds and closes the partner's view on disconnect. The two-session
  smoke now uses the non-destructive `$trade-fixture` when a 410003 already
  exists, and confirms item/silver audit entries. Session reconnect handling
  remains. Player shops now have the first server-authoritative slice: booth lifecycle, up to ten listings, 1108 vendor records, same-map query, atomic UID/silver purchase, disconnect cleanup, configurable client price, a two-session `--player-shop` probe, and a client Market panel with click/drag listing and purchase; booth names, persistence and economic logs beyond shop operations remain.
- [~] Add periodic autosave and audit logs for items and currencies. Connected characters are now persisted every 30 seconds without being removed from the world; `economic-audit.log` is centralized and covers NPC buy/sell/repair, player-shop operations, quest and monster rewards, training, gem composition, trade and warehouse silver transfers. Atomic tests now cover insufficient balance and `u32` overflow for warehouse transfers and direct rewards; the `--warehouse` probe covers the real item/silver protocol path; a full economic stress audit remains.
- [~] Start the guild foundation: the server now has an authoritative `GuildManager` with creation, invitations, leadership transfer, member removal, rank restoration and a 50-member cap; migrations `30_guilds.sql` and `31_guild_warehouse_items.sql` prepare guilds, members, guild silver and 40 item slots; guilds/members load from and persist to SQLite; `$create-guild <name>` creates a persistent test guild; `$guild-deposit <uid> <slot>` and `$guild-withdraw <uid> <slot>` exercise atomic UID-preserving item transfers, including plus/sockets/gems. Packet 2034 opens/deposits/withdraws the Guild warehouse, allows deposits to members and restricts withdrawals to the leader; the client reuses the warehouse modal with drag-and-drop and slot updates. Classic `1107` covers invite/accept/leave/kick/name-query and `2102` sends one online member record per packet. Advanced roles, war and ranking remain.
- Add administration commands, rate limits, reconnect handling, and load tests. The probe command `--load <usuário> <senha>` now covers two simultaneous logins, map movement, monster discovery, combat, death/drop/pickup; the `benchbot` smoke covers 10/25/50 remote entities and `--reconnect` confirmed same-character map reload and movement under 50 bots. A concurrent 50-bot run with two reconnect rounds per bot also passed 150 world logins; heavier concurrency remains.
- [x] Add explicit reconnect/map smoke coverage: re-authentication restores the same character ID and movement state, while the probe validates a 1002→1000→1002 transition without duplicating items or characters.
- [x] Add the reference gap matrix in `docs/reference-gap-matrix.md`, covering ReduxConquerProject and Albetros1.1 systems and the implementation order.

## Reference Mapping

| Behavior | Redux reference | Gravak status |
| --- | --- | --- |
| Physical interaction | `Packets/Game/[1022] InteractPacket.cs` | Rust wire packet and handler implemented |
| Combat rules | `Managers/CombatManager.cs`, `Objects/Entity.cs` | Basic PvE rules implemented; advanced rules pending |
| Monster entity | `Objects/Monster.cs`, `Database/Domain/DbMonstertype.cs` | Rust entity, spawn, AI and respawn implemented |
| Skill effects | `Packets/Game/[1105] SkillEffect.cs` | 1105 follows the legacy coordinate/32-byte-target layout, supports multiple targets, and is exercised by Flame Burst |
| Player trade/shop | `Managers/Trade.cs`, `GameServer.cs` | `1056` trade session with double confirmation invalidation, 60-second timeout and atomic item/silver exchange; player shop lifecycle with up to ten listings, `1108`, atomic purchase, disconnect cleanup, configurable client price, two-session probes, client Market panel and economic audit implemented; reconnect, booth names and persistence pending |
| Guilds | `Managers/GuildManager.cs`, `Structures/Guild.cs` | Domain manager, migration foundation, SQLite guild/member persistence, `1107` actions, `2102` online member records and initial Guild panel implemented; complete management actions, warehouse, war and ranking pending |
| Warehouse | `Managers/WarehouseManager.cs` | Basic Rust version implemented |
| NPC dialogs | `Npcs/` and task handlers | Basic Rust version implemented |
| Plus/gems/sockets/refinery/artifacts | item handlers, `Gem1/Gem2`, compose services, refinery packet | plus, quality, sockets, compose, Dragon/Tortoise bonuses, refinery `2076`, artifact `2076/1`, temporary persistence, Intensification/Penetration, fire resistance, character-targeted magic, PvP smoke and `2077` implemented; remaining elements and other gem/refinery effects pending |

## External 5065 References

The public `luckymouse0/Redux-Conquer-Online-Server` fork confirms the
following 5065-oriented boundaries and is useful for protocol comparison:

- `Packets/Game/[1022] InteractPacket.cs` for physical and skill interaction;
- `Packets/Game/[1014] SpawnEntity.cs` and `Packets/Game/[1109] Sob Spawn.cs`
    for world entity visibility;
- `Database/Domain/DbMonstertype.cs` and `Database/Domain/DbSpawn.cs` for
    monster definitions and map spawn rules;
- `Objects/Monster.cs`, `Managers/CombatManager.cs`,
    `Managers/SpawnManager.cs`, `Threading/CombatThread.cs`, and
    `Threading/MonsterThread.cs` for the gameplay lifecycle;
- `Packets/Game/[1101] GroundItem.cs`, `[1103] ConquerSkill.cs`, and
    `[1105] SkillEffect.cs` for the post-hit progression path.

These references reinforce the Rust implementation order: entity and spawn
data first, then interaction validation, damage, visibility, drops, and
skills. The reference code is legacy C# and should inform behavior and packet
fixtures, not be copied as server architecture.

## Implementation Rule

Each feature should be delivered as a complete vertical slice: database/data
model, server-authoritative rules, packet fixtures, client rendering, and
focused tests. Do not add a packet facade that cannot be exercised by the
client.
