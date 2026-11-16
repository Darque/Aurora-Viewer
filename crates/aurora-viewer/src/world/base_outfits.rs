//! Reusable appearance bases. Folder links are resolved before a wear: the
//! server stores links, but does not expand them into the Current Outfit.

use super::{appearance, inventory::Inventory};
use aurora_net::outfits::OutfitLink;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Why a base could not be resolved. A failed resolution must not wear a
/// partial base, since that could detach items from the current appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    Missing,
    Loading,
    Forbidden,
    Cycle,
}

fn resolvable_folder(inv: &Inventory, id: Uuid) -> bool {
    let my_outfits = super::outfit_folders::my_outfits(inv);
    inv.folders.get(&id).is_some_and(|folder| {
        !folder.library
            && Some(id) != my_outfits
            && (super::outfit_folders::in_my_outfits(inv, id) || super::outfit_folders::container(inv, id))
    })
}

/// Expand physical children and folder links, refusing cycles even when a
/// different viewer created them. A finished folder can be visited again:
/// sharing a descendant is not a cycle. The walk is iterative so malformed
/// inventory cannot exhaust the stack. Each original is returned once.
pub fn resolve(inv: &Inventory, root: Uuid) -> Result<Vec<OutfitLink>, ResolveError> {
    resolve_inner(inv, root, None)
}

fn resolve_inner(inv: &Inventory, root: Uuid, forbidden: Option<Uuid>) -> Result<Vec<OutfitLink>, ResolveError> {
    enum Visit {
        Enter(Uuid),
        Leave(Uuid),
    }
    let mut pending = vec![Visit::Enter(root)];
    let mut active = HashSet::new();
    let mut complete = HashSet::new();
    let mut targets = HashSet::new();
    let mut result = Vec::new();
    while let Some(visit) = pending.pop() {
        let id = match visit {
            Visit::Leave(id) => {
                active.remove(&id);
                complete.insert(id);
                continue;
            }
            Visit::Enter(id) => id,
        };
        if forbidden == Some(id) || active.contains(&id) {
            return Err(ResolveError::Cycle);
        }
        if complete.contains(&id) {
            continue;
        }
        let folder = inv.folders.get(&id).ok_or(ResolveError::Missing)?;
        if !resolvable_folder(inv, id) || inv.in_folder_type(id, 14) || inv.in_folder_type(id, 50) {
            return Err(ResolveError::Forbidden);
        }
        if !appearance::complete(inv, id) {
            return Err(ResolveError::Loading);
        }
        active.insert(id);
        pending.push(Visit::Leave(id));
        let mut children = folder.children.clone();
        for link in appearance::folder_links(inv, id) {
            if link.folder {
                children.push(link.target);
            } else if inv.broken(&link.target) {
                return Err(ResolveError::Missing);
            } else if let Some(item) = inv.items.get(&link.target) {
                if matches!(item.asset_type, 24 | 25) {
                    // Do not silently omit an unresolved link-to-link.
                    return Err(ResolveError::Loading);
                }
                if matches!(item.asset_type, 5 | 6 | 13 | 21) {
                    // Apply the same can_move_to_outfit rule used by inventory
                    // paste (llinventoryfunctions.cpp), including coalesced objects.
                    if !super::outfit_paste::wearable(inv, link.target, false) {
                        return Err(ResolveError::Forbidden);
                    }
                    if targets.insert(link.target) {
                        result.push(link);
                    }
                }
            }
        }
        pending.extend(children.into_iter().rev().map(Visit::Enter));
    }
    Ok(result)
}

/// Request only the selected folders' reachable contents and missing item
/// originals. This walk is cycle-safe and is repeated as inventory arrives.
pub fn graph(inv: &Inventory, roots: &[Uuid]) -> (Vec<Uuid>, HashSet<Uuid>) {
    let mut pending = roots.to_vec();
    let mut seen = HashSet::new();
    let mut folders = Vec::new();
    let mut missing = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(folder) = inv.folders.get(&id) else { continue };
        if !resolvable_folder(inv, id) || inv.in_folder_type(id, 14) || inv.in_folder_type(id, 50) {
            continue;
        }
        folders.push(id);
        pending.extend(folder.children.iter().copied());
        pending.extend(
            appearance::folder_links(inv, id)
                .into_iter()
                .filter(|link| link.folder)
                .map(|link| link.target),
        );
        missing.extend(appearance::missing_targets(inv, id));
    }
    (folders, missing)
}

pub fn prepare(inv: &mut Inventory, roots: &[Uuid], agent: Uuid) -> Vec<aurora_net::NetCommand> {
    let (folders, missing) = graph(inv, roots);
    for id in folders {
        if inv.queue.len() >= 16 {
            break;
        }
        inv.request(id);
    }
    appearance::fetch_items(missing, agent)
}

fn base_target(inv: &Inventory, id: Uuid, known: &HashSet<Uuid>) -> bool {
    known.contains(&id) || is_base_folder(inv, id)
}

/// A leading period marks a saved outfit as a reusable base. The marker is
/// inventory metadata, so another viewer installation discovers the same
/// bases without a local catalogue.
pub fn is_base_folder(inv: &Inventory, id: Uuid) -> bool {
    inv.folders.get(&id).is_some_and(|folder| {
        !folder.library
            && folder.info.type_default == appearance::FT_OUTFIT
            && folder.info.name.starts_with('.')
            && super::outfit_folders::in_my_outfits(inv, id)
    })
}

/// The inventory keeps the leading period as the base marker. Appearance
/// views use the folder name without that marker.
pub fn display_name(inv: &Inventory, id: Uuid) -> Option<&str> {
    inv.folders.get(&id).map(|folder| {
        let name = folder.info.name.as_str();
        if is_base_folder(inv, id) {
            name.strip_prefix('.').unwrap_or(name)
        } else {
            name
        }
    })
}

pub fn folders(inv: &Inventory) -> HashSet<Uuid> {
    super::outfit_folders::folders(inv)
        .into_iter()
        .filter(|id| is_base_folder(inv, *id))
        .collect()
}

/// Folder targets that an inventory category still links. The context menu
/// uses this cached set to mirror the cannot-unmark guard before conversion.
pub fn referenced(inv: &Inventory) -> HashSet<Uuid> {
    inv.folders
        .keys()
        .flat_map(|folder| appearance::folder_links(inv, *folder))
        .filter(|link| link.folder)
        .map(|link| link.target)
        .collect()
}

/// The first reusable base linked from a saved clothing outfit. Ordinary
/// folder links survive a different installation's empty local catalog.
pub fn linked_bases(inv: &Inventory, outfit: Uuid, known: &HashSet<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    appearance::folder_links(inv, outfit)
        .into_iter()
        .filter(|link| link.folder && base_target(inv, link.target, known) && seen.insert(link.target))
        .map(|link| link.target)
        .collect()
}

pub fn linked_names(inv: &Inventory, outfit: Uuid, known: &HashSet<Uuid>) -> Option<String> {
    let names: Vec<_> = linked_bases(inv, outfit, known)
        .into_iter()
        .filter_map(|id| display_name(inv, id).map(str::to_owned))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// Set an ordinary inventory folder link on a clothing outfit. Reject a
/// link if the target already reaches the outfit, even through physical
/// children or existing folder links. Recheck the graph when wearing it.
pub fn set_link(
    inv: &Inventory,
    outfit: Uuid,
    target: Option<Uuid>,
    bases: &HashSet<Uuid>,
) -> Result<aurora_net::outfits::OutfitMutation, ResolveError> {
    if !super::outfit_folders::in_my_outfits(inv, outfit) || bases.contains(&outfit) {
        return Err(ResolveError::Forbidden);
    }
    if !appearance::complete(inv, outfit) {
        return Err(ResolveError::Loading);
    }
    let mut links = appearance::folder_links(inv, outfit);
    let existing = links
        .iter()
        .any(|link| link.folder && target == Some(link.target) && base_target(inv, link.target, bases));
    if let Some(target) = target {
        if !bases.contains(&target) {
            return Err(ResolveError::Forbidden);
        }
        if !existing {
            resolve_inner(inv, target, Some(outfit))?;
        }
    }
    links.retain(|link| !link.folder || !base_target(inv, link.target, bases) || target.is_some_and(|target| target != link.target));
    if let Some(target) = target {
        if !existing {
            let folder = inv.folders.get(&target).ok_or(ResolveError::Missing)?;
            links.push(OutfitLink {
                target,
                name: folder.info.name.clone(),
                desc: String::new(),
                folder: true,
                inv_type: 8,
            });
        }
    }
    Ok(aurora_net::outfits::OutfitMutation {
        folder: outfit,
        create: None,
        links,
        cof: None,
    })
}

/// The base inferred from the confirmed Current Outfit. This is kept only in
/// memory; inventory folder names identify bases across sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Active {
    /// A representative active base for older appearance-window workflows.
    pub folder: Uuid,
    /// The confirmed contents attributed to each active base.
    pub members: HashMap<Uuid, BaseItems>,
    /// Union of all active base items, retained for the shared appearance UI.
    pub items: HashSet<Uuid>,
    /// Union of active-base items not retained by the current clothing outfit.
    pub exclusive: HashSet<Uuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaseItems {
    pub items: HashSet<Uuid>,
    pub exclusive: HashSet<Uuid>,
}

impl Active {
    pub fn folders(&self) -> Vec<Uuid> {
        let mut folders: Vec<_> = self.members.keys().copied().collect();
        if folders.is_empty() && !self.folder.is_nil() {
            folders.push(self.folder);
        }
        folders.sort_unstable();
        folders
    }

    pub fn contains(&self, folder: Uuid) -> bool {
        self.members.contains_key(&folder) || self.folder == folder
    }

    pub fn retain_items(&mut self, mut keep: impl FnMut(&Uuid) -> bool) {
        for member in self.members.values_mut() {
            member.items.retain(|id| keep(id));
            member.exclusive.retain(|id| keep(id) && member.items.contains(id));
        }
        self.members.retain(|_, member| !member.items.is_empty());
        if let Some(active) = Self::from_members(self.members.clone(), Some(self.folder)) {
            *self = active;
        } else {
            *self = Self::default();
        }
    }

    fn from_members(members: HashMap<Uuid, BaseItems>, preferred: Option<Uuid>) -> Option<Self> {
        if members.is_empty() {
            return None;
        }
        let mut folders: Vec<_> = members.keys().copied().collect();
        folders.sort_unstable();
        let folder = preferred.filter(|id| members.contains_key(id)).unwrap_or(folders[0]);
        let items = members.values().flat_map(|member| member.items.iter().copied()).collect();
        let exclusive = members.values().flat_map(|member| member.exclusive.iter().copied()).collect();
        Some(Self {
            folder,
            members,
            items,
            exclusive,
        })
    }
}

/// The discovered bases and current base. Only the generation is cached;
/// active membership is rebuilt from confirmed inventory links.
#[derive(Debug, Clone, Default)]
pub struct Saved {
    pub folders: HashSet<Uuid>,
    pub referenced: HashSet<Uuid>,
    pub active: Option<Active>,
    generation: u64,
}

pub struct Store {
    pub data: Saved,
}

impl Default for Store {
    fn default() -> Self {
        Self { data: Saved::default() }
    }
}

impl Store {
    pub fn refresh(&mut self, inv: &Inventory) {
        if self.data.generation == inv.generation {
            return;
        }
        let previous = self.data.active.clone();
        self.data.folders = folders(inv);
        self.data.referenced = referenced(inv);
        self.data.active = current(inv, &self.data.folders, previous.as_ref());
        self.data.generation = inv.generation;
    }

    pub fn refresh_now(&mut self, inv: &Inventory) {
        self.data.generation = u64::MAX;
        self.refresh(inv);
    }
}

/// Infer the base from the items actually linked in Current Outfit. If two
/// bases have an identical worn signature, prefer the in-memory previous
/// choice, then the current saved outfit's base link; otherwise leave it
/// unknown rather than removing items from the wrong base.
pub fn current(inv: &Inventory, bases: &HashSet<Uuid>, previous: Option<&Active>) -> Option<Active> {
    let previous = previous.filter(|active| active.folders().iter().any(|id| bases.contains(id)));
    let cof = appearance::cof(inv)?;
    if !appearance::complete(inv, cof) {
        return previous.cloned();
    }
    let worn: HashSet<_> = appearance::folder_links(inv, cof)
        .into_iter()
        .filter(|link| !link.folder)
        .map(|link| link.target)
        .collect();
    let clothing = appearance::base(inv).map(|id| clothing_items(inv, id)).unwrap_or_default();
    let linked: HashSet<_> = appearance::base(inv)
        .map(|id| linked_bases(inv, id, bases))
        .unwrap_or_default()
        .into_iter()
        .collect();
    let mut ranked = Vec::new();
    let mut loading = false;
    for id in bases {
        let items = match resolve(inv, *id) {
            Ok(items) => items,
            Err(ResolveError::Loading) => {
                loading = true;
                continue;
            }
            Err(_) => continue,
        };
        let items: HashSet<_> = items
            .into_iter()
            .map(|link| link.target)
            .filter(|item| worn.contains(item))
            .collect();
        if items.is_empty() {
            continue;
        }
        let body_parts = items.iter().filter(|item| body_slot(inv, **item).is_some()).count();
        ranked.push((*id, (body_parts, items.len()), items));
    }
    if ranked.is_empty() {
        return if loading { previous.cloned() } else { None };
    }
    ranked.sort_by(|(left, left_score, _), (right, right_score, _)| {
        let retained = |id: &Uuid| previous.is_some_and(|active| active.members.contains_key(id));
        let linked = |id: &Uuid| linked.contains(id);
        retained(right)
            .cmp(&retained(left))
            .then_with(|| right_score.cmp(left_score))
            .then_with(|| linked(right).cmp(&linked(left)))
            .then_with(|| left.cmp(right))
    });
    let mut members = HashMap::new();
    let mut claimed = HashSet::new();
    let mut slots = HashSet::new();
    for (id, _, items) in ranked {
        let unique: HashSet<_> = items
            .into_iter()
            .filter(|item| !claimed.contains(item))
            .filter(|item| body_slot(inv, *item).is_none_or(|slot| !slots.contains(&slot)))
            .collect();
        if unique.is_empty() {
            continue;
        }
        claimed.extend(unique.iter().copied());
        slots.extend(unique.iter().filter_map(|item| body_slot(inv, *item)));
        let mut exclusive: HashSet<_> = unique.difference(&clothing).copied().collect();
        if let Some(previous) = previous.and_then(|active| active.members.get(&id)) {
            exclusive.extend(previous.exclusive.intersection(&unique).copied());
        }
        members.insert(id, BaseItems { items: unique, exclusive });
    }
    let preferred = previous
        .map(|active| active.folder)
        .filter(|id| members.contains_key(id))
        .or_else(|| linked.iter().find(|id| members.contains_key(id)).copied());
    Active::from_members(members, preferred).or_else(|| if loading { previous.cloned() } else { None })
}

fn clothing_items(inv: &Inventory, root: Uuid) -> HashSet<Uuid> {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    let mut items = HashSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(folder) = inv.folders.get(&id) else { continue };
        pending.extend(folder.children.iter().copied());
        items.extend(
            appearance::folder_links(inv, id)
                .into_iter()
                .filter(|link| !link.folder)
                .map(|link| link.target),
        );
    }
    items
}

fn body_slot(inv: &Inventory, id: Uuid) -> Option<u32> {
    inv.items
        .get(&id)
        .filter(|item| item.asset_type == 13)
        .map(|item| item.flags & 0xff)
}

/// Preserve the confirmed worn items of active bases during a clothing
/// replacement. An explicit new body part wins its single slot, as in
/// `LLAppearanceMgr::updateCOF` (indra/newview/llappearancemgr.cpp).
pub fn preserve(inv: &Inventory, current: &[OutfitLink], next: &mut Vec<OutfitLink>, base: &Active) {
    for old in current.iter().filter(|link| !link.folder && base.items.contains(&link.target)) {
        if next.iter().any(|link| !link.folder && link.target == old.target) {
            continue;
        }
        if let Some(slot) = body_slot(inv, old.target)
            && next.iter().any(|link| !link.folder && body_slot(inv, link.target) == Some(slot))
        {
            continue;
        }
        next.push(old.clone());
    }
}

/// Merge linked bases into the new clothing links. The ordinary outfit
/// planner keeps required old body parts, so remove the previous base's
/// items and let the linked base replace matching body slots before saving
/// the one resulting Current Outfit.
fn replace_for_clothing(
    inv: &Inventory,
    next: &mut Vec<OutfitLink>,
    previous: Option<&Active>,
    incoming: &[OutfitLink],
    clothing: &HashSet<Uuid>,
) {
    let incoming_ids: HashSet<_> = incoming.iter().filter(|link| !link.folder).map(|link| link.target).collect();
    let incoming_slots: HashSet<_> = incoming_ids.iter().filter_map(|id| body_slot(inv, *id)).collect();
    next.retain(|link| {
        if link.folder || incoming_ids.contains(&link.target) || clothing.contains(&link.target) {
            return true;
        }
        if body_slot(inv, link.target).is_some_and(|slot| incoming_slots.contains(&slot)) {
            return false;
        }
        !previous.is_some_and(|base| base.items.contains(&link.target) && !clothing.contains(&link.target))
    });
    let mut held: HashSet<_> = next.iter().filter(|link| !link.folder).map(|link| link.target).collect();
    next.extend(incoming.iter().filter(|link| !link.folder && held.insert(link.target)).cloned());
}

/// Compare the clothing outfit without treating the separately worn base as
/// unsaved clothing. Normalise layer numbers after removing base-only items,
/// preserving relative order when the base has layers of the same type.
pub fn dirty(inv: &Inventory, active: Option<&Active>) -> bool {
    let Some(active) = active else { return appearance::dirty(inv) };
    let Some((cof, clothing)) = appearance::cof(inv).zip(appearance::base(inv)) else {
        return false;
    };
    if !appearance::loaded(inv, clothing) {
        return false;
    }
    let wearable = |link: &OutfitLink| {
        !link.folder
            && inv
                .items
                .get(&link.target)
                .is_some_and(|item| matches!(item.asset_type, 5 | 6 | 13 | 21))
    };
    let mut expected: Vec<_> = appearance::folder_links(inv, clothing).into_iter().filter(&wearable).collect();
    let clothing_ids: HashSet<_> = expected.iter().map(|link| link.target).collect();
    let mut actual: Vec<_> = appearance::folder_links(inv, cof)
        .into_iter()
        .filter(&wearable)
        .filter(|link| !active.items.contains(&link.target) || clothing_ids.contains(&link.target))
        .collect();
    appearance::normalize_layers(inv, &mut expected);
    appearance::normalize_layers(inv, &mut actual);
    let keys = |links: Vec<OutfitLink>| {
        let mut keys: Vec<_> = links.into_iter().map(|link| (link.target, link.name, link.desc)).collect();
        keys.sort();
        keys
    };
    keys(actual) != keys(expected)
}

/// Plan a base swap against the confirmed Current Outfit. Items known to
/// the clothing outfit remain on; a base's exclusive items
/// are removed unless the incoming base or the clothing outfit needs them.
/// Body parts are never removed without a replacement for their slot.
#[cfg(test)]
pub fn swap(
    inv: &Inventory,
    current: &[OutfitLink],
    previous: Option<&Active>,
    folder: Uuid,
    incoming: &[OutfitLink],
    clothing: &HashSet<Uuid>,
) -> (Vec<OutfitLink>, Active) {
    let (links, active) = swap_many(inv, current, previous, &[(folder, incoming.to_vec())], clothing, false);
    (links, active.unwrap_or_default())
}

/// Add or replace several bases in one Current Outfit update. A base replaces
/// only active bases with overlapping body-part or attachment
/// slots; unrelated bases remain active, so a hair base can coexist with a
/// body base. An empty list clears the tracked base set.
pub fn swap_many(
    inv: &Inventory,
    current: &[OutfitLink],
    previous: Option<&Active>,
    incoming_bases: &[(Uuid, Vec<OutfitLink>)],
    clothing: &HashSet<Uuid>,
    replace_all: bool,
) -> (Vec<OutfitLink>, Option<Active>) {
    let mut prior = previous.map(|active| active.members.clone()).unwrap_or_default();
    if prior.is_empty()
        && let Some(active) = previous.filter(|active| !active.folder.is_nil())
    {
        prior.insert(
            active.folder,
            BaseItems {
                items: active.items.clone(),
                exclusive: active.exclusive.clone(),
            },
        );
    }
    let normalized: Vec<_> = incoming_bases
        .iter()
        .map(|(folder, links)| (*folder, normalize_base_links(inv, links)))
        .collect();
    let replaced: HashSet<_> = prior
        .iter()
        .filter(|(folder, member)| {
            replace_all
                || incoming_bases.is_empty()
                || normalized.iter().any(|(incoming_folder, links)| {
                    **folder == *incoming_folder || signatures_overlap(inv, &member.items, &links.iter().map(|link| link.target).collect())
                })
        })
        .map(|(folder, _)| *folder)
        .collect();
    let removed: HashSet<_> = prior
        .iter()
        .filter(|(folder, _)| replaced.contains(folder))
        .flat_map(|(_, member)| member.exclusive.iter().copied())
        .filter(|id| !clothing.contains(id))
        .collect();
    let mut members: HashMap<_, _> = prior.into_iter().filter(|(folder, _)| !replaced.contains(folder)).collect();
    let incoming: Vec<_> = normalize_base_links(inv, &normalized.iter().flat_map(|(_, links)| links.clone()).collect::<Vec<_>>());
    let incoming_ids: HashSet<_> = incoming.iter().map(|link| link.target).collect();
    let incoming_slots: HashSet<_> = incoming_ids.iter().filter_map(|id| body_slot(inv, *id)).collect();
    let mut next: Vec<_> = current
        .iter()
        .filter(|link| {
            if link.folder || incoming_ids.contains(&link.target) || clothing.contains(&link.target) {
                return true;
            }
            // Clearing base membership does not leave the avatar without a
            // required body-part slot; only a different body part may replace it.
            if replace_all && incoming.is_empty() && body_slot(inv, link.target).is_some() {
                return true;
            }
            if body_slot(inv, link.target).is_some_and(|slot| incoming_slots.contains(&slot)) {
                return false;
            }
            !removed.contains(&link.target)
        })
        .cloned()
        .collect();
    let mut held: HashSet<_> = next.iter().filter(|link| !link.folder).map(|link| link.target).collect();
    next.extend(incoming.iter().filter(|link| !link.folder && held.insert(link.target)).cloned());

    let worn: HashSet<_> = next.iter().filter(|link| !link.folder).map(|link| link.target).collect();
    for (folder, links) in normalized {
        let items: HashSet<_> = links.into_iter().map(|link| link.target).filter(|id| worn.contains(id)).collect();
        let exclusive = items.difference(clothing).copied().collect();
        members.insert(folder, BaseItems { items, exclusive });
    }
    for member in members.values_mut() {
        member.items.retain(|id| worn.contains(id));
        member.exclusive.retain(|id| member.items.contains(id));
    }
    let active = Active::from_members(
        members,
        incoming_bases
            .first()
            .map(|(folder, _)| *folder)
            .or_else(|| previous.map(|a| a.folder)),
    );
    (next, active)
}

fn normalize_base_links(inv: &Inventory, links: &[OutfitLink]) -> Vec<OutfitLink> {
    let mut links: Vec<_> = links.iter().filter(|link| !link.folder).cloned().collect();
    let mut slots = HashSet::new();
    let mut unique = HashSet::new();
    links.reverse();
    links.retain(|link| unique.insert(link.target) && body_slot(inv, link.target).is_none_or(|slot| slots.insert(slot)));
    links.reverse();
    links
}

fn signatures_overlap(inv: &Inventory, left: &HashSet<Uuid>, right: &HashSet<Uuid>) -> bool {
    fn signature(inv: &Inventory, ids: &HashSet<Uuid>) -> HashSet<(u8, u32)> {
        ids.iter()
            .filter_map(|id| {
                let item = inv.items.get(id)?;
                match item.asset_type {
                    13 => Some((13, item.flags & 0xff)),
                    6 => Some((6, item.flags & 0xff)),
                    _ => None,
                }
            })
            .collect()
    }
    !signature(inv, left).is_disjoint(&signature(inv, right))
}

pub fn error_text(error: ResolveError) -> String {
    match error {
        ResolveError::Cycle => aurora_i18n::tr!("world-appearance-cycle-in-outfit-folders"),
        ResolveError::Loading => aurora_i18n::tr!("world-appearance-this-outfit-s-inventory-is-still-loading"),
        ResolveError::Missing => aurora_i18n::tr!("common-folder-not-found"),
        ResolveError::Forbidden => aurora_i18n::tr!("world-appearance-this-outfit-folder-cannot-be-modified"),
    }
    .into()
}

fn check_limits(inv: &Inventory, links: &[OutfitLink]) -> Result<(), String> {
    if links
        .iter()
        .filter(|link| !link.folder && inv.items.get(&link.target).is_some_and(|item| item.asset_type == 6))
        .count()
        > 38
    {
        return Err(aurora_i18n::tr!("world-appearance-the-outfit-exceeds-the-38-attachment-limit").into());
    }
    for kind in 4..=16 {
        if links
            .iter()
            .filter(|link| {
                !link.folder
                    && inv
                        .items
                        .get(&link.target)
                        .is_some_and(|item| item.asset_type == 5 && item.flags & 0xff == kind)
            })
            .count()
            > 60
        {
            return Err(aurora_i18n::tr!("world-appearance-the-outfit-exceeds-the-limit-of-60-layers-per").into());
        }
    }
    Ok(())
}

/// Applies active bases in the same single Current Outfit update as a
/// clothing wear, so attachments and RLVa restrictions use the usual path.
pub fn plan(
    inv: &Inventory,
    action: appearance::Action,
    worn: &HashMap<Uuid, u8>,
    saved: &Saved,
) -> Result<(aurora_net::outfits::OutfitMutation, bool, Option<Active>), String> {
    use appearance::Action;
    let mut saved = saved.clone();
    saved.folders = folders(inv);
    saved.active = current(inv, &saved.folders, saved.active.as_ref());
    let clearing = matches!(action, Action::ClearBase) || matches!(&action, Action::WearBases(ids) if ids.is_empty());
    let direct_bases = match &action {
        Action::WearBase(id) => Some(vec![*id]),
        Action::WearBases(ids) => Some(ids.clone()),
        Action::ClearBase => Some(Vec::new()),
        Action::Wear(id, _) if saved.folders.contains(id) => Some(vec![*id]),
        Action::WearContents { folder, .. } if saved.folders.contains(folder) => Some(vec![*folder]),
        Action::Revert => appearance::base(inv).filter(|id| saved.folders.contains(id)).map(|id| vec![id]),
        _ => None,
    };
    if matches!(&action, Action::RemoveOutfit(id) if saved.folders.contains(id))
        || matches!(&action, Action::RemoveOutfits(ids) if ids.iter().any(|id| saved.folders.contains(id)))
        || matches!(&action, Action::WearOutfits { folders, .. } if folders.iter().any(|id| saved.folders.contains(id)))
    {
        return Err(error_text(ResolveError::Forbidden));
    }
    let clothing_folders: Vec<Uuid> = match &action {
        Action::Wear(id, false) if direct_bases.is_none() => vec![*id],
        Action::WearOutfits { folders, append: false } => folders.clone(),
        Action::WearContents {
            folder,
            mode: appearance::WearMode::ReplaceOutfit,
        } if direct_bases.is_none() => vec![*folder],
        Action::Revert if direct_bases.is_none() => appearance::base(inv).into_iter().collect(),
        _ => Vec::new(),
    };
    let replace_clothing = direct_bases.is_none()
        && (!clothing_folders.is_empty()
            || matches!(
                &action,
                Action::WearContents {
                    mode: appearance::WearMode::ReplaceOutfit,
                    ..
                }
            ));
    let mut linked = Vec::new();
    let mut linked_seen = HashSet::new();
    for folder in &clothing_folders {
        if appearance::folder_links(inv, *folder)
            .iter()
            .any(|link| link.folder && !inv.folders.contains_key(&link.target))
        {
            return Err(error_text(ResolveError::Missing));
        }
        linked.extend(
            linked_bases(inv, *folder, &saved.folders)
                .into_iter()
                .filter(|id| linked_seen.insert(*id)),
        );
    }
    let linked_bases = linked;
    if direct_bases.is_none() && !clearing && !replace_clothing {
        let (mut change, mut sync) = appearance::plan(inv, action.clone(), worn)?;
        let mut updated_base = None;
        if matches!(
            action,
            Action::RemoveOutfit(_) | Action::RemoveOutfits(_) | Action::ImportJson { .. }
        ) && let Some(active) = &saved.active
            && let Some(cof) = appearance::cof(inv)
        {
            let mut clothing: HashSet<_> = if matches!(action, Action::ImportJson { .. }) {
                change.links.iter().filter(|link| !link.folder).map(|link| link.target).collect()
            } else {
                appearance::base(inv)
                    .filter(|id| !saved.folders.contains(id))
                    .map(|id| {
                        appearance::folder_links(inv, id)
                            .into_iter()
                            .filter(|link| !link.folder)
                            .map(|link| link.target)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let removed = match &action {
                Action::RemoveOutfit(id) => vec![*id],
                Action::RemoveOutfits(ids) => ids.clone(),
                _ => Vec::new(),
            };
            for id in removed {
                let links = if inv
                    .folders
                    .get(&id)
                    .is_some_and(|folder| folder.info.type_default == appearance::FT_OUTFIT)
                {
                    appearance::usable_links(inv, id)
                } else {
                    appearance::wearable_contents(inv, id)?
                };
                for link in links.into_iter().filter(|link| !link.folder) {
                    clothing.remove(&link.target);
                }
            }
            let mut updated = active.clone();
            for member in updated.members.values_mut() {
                member.exclusive = member.items.difference(&clothing).copied().collect();
            }
            updated.exclusive = updated
                .members
                .values()
                .flat_map(|member| member.exclusive.iter().copied())
                .collect();
            let current = appearance::folder_links(inv, cof);
            let links = if change.folder == cof {
                Some(&mut change.links)
            } else {
                change.cof.as_mut().filter(|(id, _)| *id == cof).map(|(_, links)| links)
            };
            if let Some(links) = links {
                preserve(inv, &current, links, active);
                appearance::drop_coalesced(inv, links);
                appearance::normalize_layers(inv, links);
                check_limits(inv, links)?;
                updated_base = Some(updated);
            }
        }
        if matches!(&action, Action::SaveTo(id) if saved.folders.contains(id)) && change.folder != appearance::cof(inv).unwrap_or_default()
        {
            // A base save snapshots the currently worn items into that folder;
            // it does not switch the Current Outfit folder or its base links.
            change.cof = None;
            sync = false;
        } else if matches!(action, Action::Save(_) | Action::SaveIn(..) | Action::SaveTo(_))
            && change.folder != appearance::cof(inv).unwrap_or_default()
        {
            let previous_clothing: HashSet<_> = appearance::base(inv)
                .filter(|id| !saved.folders.contains(id))
                .map(|id| {
                    appearance::folder_links(inv, id)
                        .into_iter()
                        .filter(|link| !link.folder)
                        .map(|link| link.target)
                        .collect()
                })
                .unwrap_or_default();
            if let Some(active) = saved
                .active
                .as_ref()
                .filter(|base| base.folders().iter().any(|id| saved.folders.contains(id)))
            {
                change
                    .links
                    .retain(|link| link.folder || !active.exclusive.contains(&link.target) || previous_clothing.contains(&link.target));
                change
                    .links
                    .retain(|link| !link.folder || !base_target(inv, link.target, &saved.folders));
                for id in active.folders() {
                    resolve_inner(inv, id, Some(change.folder)).map_err(error_text)?;
                    let folder = inv.folders.get(&id).ok_or_else(|| error_text(ResolveError::Missing))?;
                    change.links.push(OutfitLink {
                        target: id,
                        name: folder.info.name.clone(),
                        desc: String::new(),
                        folder: true,
                        inv_type: 8,
                    });
                }
            } else if change.create.is_none() {
                change.links.extend(
                    appearance::folder_links(inv, change.folder)
                        .into_iter()
                        .filter(|link| link.folder && base_target(inv, link.target, &saved.folders)),
                );
            }
        }
        return Ok((change, sync, updated_base));
    }
    if direct_bases.is_none() && !clearing && linked_bases.is_empty() && saved.active.is_none() {
        let (change, sync) = appearance::plan(inv, action, worn)?;
        return Ok((change, sync, None));
    }
    let selected = direct_bases.clone().or_else(|| (!linked_bases.is_empty()).then_some(linked_bases));
    let new_bases: Vec<_> = selected
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            if !base_target(inv, id, &saved.folders) {
                return Err(error_text(ResolveError::Forbidden));
            }
            Ok((id, resolve(inv, id).map_err(error_text)?))
        })
        .collect::<Result<_, String>>()?;
    let cof =
        appearance::cof(inv).ok_or_else(|| aurora_i18n::tr!("world-appearance-the-current-outfit-folder-is-unavailable").to_owned())?;
    if !appearance::complete(inv, cof) {
        return Err(error_text(ResolveError::Loading));
    }
    let mut current = appearance::usable_links(inv, cof);
    for id in worn.keys() {
        if !current.iter().any(|link| !link.folder && link.target == *id) {
            let item = inv
                .items
                .get(id)
                .ok_or_else(|| aurora_i18n::tr!("world-appearance-a-worn-object-is-still-loading").to_owned())?;
            current.push(appearance::item_link(item));
        }
    }
    let mut proposed_active = None;
    if direct_bases.is_some() {
        let incoming = &new_bases;
        let mut clothing: HashSet<_> = appearance::base(inv)
            .filter(|folder| saved.active.as_ref().is_none_or(|base| !base.contains(*folder)))
            .map(|folder| {
                appearance::folder_links(inv, folder)
                    .into_iter()
                    .filter(|link| !link.folder)
                    .map(|link| link.target)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(previous) = &saved.active {
            // A saved outfit may still name an item that the user took off;
            // its confirmed base-only status takes precedence over that copy.
            clothing.retain(|id| !previous.exclusive.contains(id));
        }
        let exact = matches!(&action, Action::WearBases(_) | Action::ClearBase);
        let (mut links, active) = swap_many(inv, &current, saved.active.as_ref(), incoming, &clothing, exact);
        appearance::drop_coalesced(inv, &mut links);
        appearance::normalize_layers(inv, &mut links);
        check_limits(inv, &links)?;
        return Ok((
            aurora_net::outfits::OutfitMutation {
                folder: cof,
                create: None,
                links,
                cof: None,
            },
            true,
            Some(active.unwrap_or_default()),
        ));
    }
    let (mut change, sync) = appearance::plan(inv, action, worn)?;
    if replace_clothing && change.folder == cof {
        if !new_bases.is_empty() {
            let clothing: HashSet<_> = clothing_folders.iter().flat_map(|folder| clothing_items(inv, *folder)).collect();
            let (_, active) = swap_many(inv, &current, saved.active.as_ref(), &new_bases, &clothing, false);
            if let Some(active) = active {
                let mut incoming = Vec::new();
                for folder in active.folders() {
                    incoming.extend(resolve(inv, folder).map_err(error_text)?);
                }
                replace_for_clothing(inv, &mut change.links, saved.active.as_ref(), &incoming, &clothing);
                proposed_active = Some(active);
            }
        } else if let Some(active) = &saved.active {
            let clothing: HashSet<_> = change.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
            let mut updated = active.clone();
            for member in updated.members.values_mut() {
                member.exclusive = member.items.difference(&clothing).copied().collect();
            }
            updated.exclusive = updated
                .members
                .values()
                .flat_map(|member| member.exclusive.iter().copied())
                .collect();
            preserve(inv, &current, &mut change.links, active);
            proposed_active = Some(updated);
        }
        appearance::drop_coalesced(inv, &mut change.links);
        appearance::normalize_layers(inv, &mut change.links);
        check_limits(inv, &change.links)?;
    }
    Ok((change, sync, proposed_active))
}

/// Two synthetic bases and two clothing outfits for the offline demo.
pub fn seed_demo(inv: &mut Inventory, agent: Uuid) -> Saved {
    let id = Uuid::from_u128;
    let mut variants = Vec::new();
    for (number, source, name) in [
        (730, 710, "Base B shape"),
        (731, 711, "Base B skin"),
        (732, 712, "Base B hair"),
        (733, 713, "Base B eyes"),
        (751, 717, "Hair base mesh"),
        (752, 714, "Hair base eyebrow tint"),
    ] {
        if let Some(mut item) = inv.items.get(&id(source)).cloned() {
            item.id = id(number);
            item.parent = inv.root;
            item.name = name.into();
            variants.push(item);
        }
    }
    inv.add_items(variants);
    let hair_folder = id(750);
    if let Some(template) = inv.folders.get(&id(703)).cloned() {
        let mut folder = template;
        folder.info.id = hair_folder;
        folder.info.parent = inv.folders.get(&id(703)).map_or(inv.root, |base| base.info.parent);
        folder.info.name = ".Hair Base".into();
        folder.children.clear();
        folder.items.clear();
        folder.state = super::inventory::FetchState::Fetched;
        inv.folders.insert(hair_folder, folder);
        if let Some(parent) = inv.folders.get_mut(&id(703)).map(|base| base.info.parent)
            && let Some(my_outfits) = inv.folders.get_mut(&parent)
        {
            my_outfits.children.push(hair_folder);
        }
        let hair_links: Vec<_> = [751, 752]
            .into_iter()
            .filter_map(|item| inv.items.get(&id(item)).map(appearance::item_link))
            .collect();
        appearance::demo_replace(inv, agent, hair_folder, &hair_links);
    }
    for (folder, name, items) in [
        (702, ".Base A", vec![710, 711, 712, 713, 714]),
        (703, ".Base B", vec![730, 731, 732, 733, 723]),
        (704, "Clothes with base", vec![715, 716, 720]),
        (705, "Clothes without base", vec![715, 716, 719]),
    ] {
        if let Some(f) = inv.folders.get_mut(&id(folder)) {
            f.info.name = name.into();
        }
        let mut links: Vec<_> = items
            .into_iter()
            .filter_map(|item| inv.items.get(&id(item)).map(appearance::item_link))
            .collect();
        if folder == 704 {
            links.push(OutfitLink {
                target: id(703),
                name: ".Base B".into(),
                desc: String::new(),
                folder: true,
                inv_type: 8,
            });
            links.push(OutfitLink {
                target: hair_folder,
                name: ".Hair Base".into(),
                desc: String::new(),
                folder: true,
                inv_type: 8,
            });
        }
        appearance::demo_replace(inv, agent, id(folder), &links);
    }
    let cof = appearance::cof(inv).unwrap_or_else(|| id(701));
    let mut current_links: Vec<_> = resolve(inv, id(702)).unwrap_or_default();
    current_links.extend(appearance::folder_links(inv, id(705)).into_iter().filter(|link| !link.folder));
    current_links.extend(
        [718, 720, 721]
            .into_iter()
            .filter_map(|item| inv.items.get(&id(item)).map(appearance::item_link)),
    );
    current_links.push(OutfitLink {
        target: id(705),
        name: "Clothes without base".into(),
        desc: String::new(),
        folder: true,
        inv_type: 8,
    });
    appearance::demo_replace(inv, agent, cof, &current_links);
    let known = folders(inv);
    Saved {
        folders: known.clone(),
        referenced: referenced(inv),
        active: current(inv, &known, None),
        generation: inv.generation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::appearance::{demo_replace, folder_links, seed_demo};

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn inventory() -> Inventory {
        let mut inv = Inventory::default();
        seed_demo(&mut inv, id(1));
        inv
    }

    fn add_link(inv: &mut Inventory, source: Uuid, target: Uuid) {
        let mut links = folder_links(inv, source);
        links.push(OutfitLink {
            target,
            name: "Base".into(),
            desc: String::new(),
            folder: true,
            inv_type: 8,
        });
        demo_replace(inv, id(1), source, &links);
    }

    #[test]
    fn outfit_replacement_keeps_the_current_base_and_base_swap_keeps_clothing() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (clothes, _, _) = plan(&inv, appearance::Action::Wear(id(705), false), &HashMap::new(), &saved).expect("wear clothes");
        let targets: HashSet<_> = clothes.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [710, 711, 712, 713, 714, 715, 716, 719] {
            assert!(targets.contains(&id(item)), "missing {item}");
        }
        let (linked, _, active) =
            plan(&inv, appearance::Action::Wear(id(704), false), &HashMap::new(), &saved).expect("wear linked clothes");
        let linked_targets: HashSet<_> = linked.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [730, 731, 732, 733, 715, 716, 720, 723, 751, 752] {
            assert!(linked_targets.contains(&id(item)), "linked outfit is missing {item}");
        }
        for item in [710, 711, 712, 713, 714] {
            assert!(!linked_targets.contains(&id(item)), "old base item {item} must be replaced");
        }
        for slot in 0..4 {
            assert_eq!(
                linked_targets.iter().filter(|item| body_slot(&inv, **item) == Some(slot)).count(),
                1,
                "body slot {slot} has exactly one wearable"
            );
        }
        let active = active.expect("linked bases");
        assert_eq!(active.folder, id(703));
        assert_eq!(HashSet::from_iter(active.folders()), HashSet::from([id(703), id(750)]));
        let (base, _, active) = plan(&inv, appearance::Action::WearBase(id(703)), &HashMap::new(), &saved).expect("swap base");
        let targets: HashSet<_> = base.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [730, 731, 732, 733, 715, 716, 719, 723] {
            assert!(targets.contains(&id(item)), "missing {item}");
        }
        assert!(!targets.contains(&id(710)) && !targets.contains(&id(711)) && !targets.contains(&id(714)) && !targets.contains(&id(717)));
        assert_eq!(active.expect("new base").folder, id(703));
    }

    #[test]
    fn a_hair_base_can_be_added_without_replacing_the_body_base() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, _, active) = plan(&inv, appearance::Action::WearBase(id(750)), &HashMap::new(), &saved).expect("wear hair base");
        let targets: HashSet<_> = change.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [710, 711, 712, 713, 714, 751, 752, 715, 716, 719] {
            assert!(targets.contains(&id(item)), "missing {item}");
        }
        let active = active.expect("body and hair bases");
        assert!(active.contains(id(702)) && active.contains(id(750)));
        assert_eq!(active.items.len(), 7);
    }

    #[test]
    fn one_outfit_links_and_applies_body_and_hair_bases_together() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let linked = linked_bases(&inv, id(704), &saved.folders);
        assert_eq!(linked, vec![id(703), id(750)]);
        let (change, _, active) = plan(&inv, appearance::Action::Wear(id(704), false), &HashMap::new(), &saved).expect("wear outfit");
        let targets: HashSet<_> = change.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [730, 731, 732, 733, 723, 751, 752, 715, 716, 720] {
            assert!(targets.contains(&id(item)), "missing linked base item {item}");
        }
        for item in [710, 711, 712, 713, 714] {
            assert!(!targets.contains(&id(item)), "old body base item {item} remains");
        }
        let active = active.expect("two active bases");
        assert_eq!(HashSet::from_iter(active.folders()), HashSet::from([id(703), id(750)]));
    }

    #[test]
    fn period_prefixed_folders_are_discovered_and_active_base_is_inferred() {
        let mut inv = inventory();
        super::seed_demo(&mut inv, id(1));
        let mut state = Store::default();
        state.refresh_now(&inv);
        assert_eq!(state.data.folders, HashSet::from([id(702), id(703), id(750)]));
        assert_eq!(state.data.active.as_ref().map(|base| base.folder), Some(id(702)));
        assert_eq!(state.data.active.as_ref().map(|base| base.items.len()), Some(5));
        inv.folders.get_mut(&id(702)).expect("base").info.name = "Base A".into();
        inv.generation += 1;
        state.refresh(&inv);
        assert_eq!(state.data.folders, HashSet::from([id(703), id(750)]));
        assert_eq!(state.data.active, None, "renaming away the marker stops tracking it as a base");
    }

    #[test]
    fn appearance_names_hide_the_inventory_base_marker() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        assert_eq!(display_name(&inv, id(702)), Some("Base A"));
        assert_eq!(display_name(&inv, id(703)), Some("Base B"));
        assert_eq!(display_name(&inv, id(704)), Some("Clothes with base"));
        assert_eq!(linked_names(&inv, id(704), &saved.folders).as_deref(), Some("Base B, Hair Base"));
        assert_eq!(display_name(&inv, id(9999)), None);
    }

    #[test]
    fn clearing_a_base_keeps_clothing_and_required_body_parts() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, _, active) = plan(&inv, appearance::Action::ClearBase, &HashMap::new(), &saved).expect("clear base");
        let targets: HashSet<_> = change.links.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for item in [710, 711, 712, 713, 715, 716, 719] {
            assert!(targets.contains(&id(item)), "missing {item}");
        }
        assert!(!targets.contains(&id(714)));
        assert!(active.expect("clear marker").folder.is_nil());
    }

    #[test]
    fn an_item_shared_with_old_clothing_becomes_base_only_after_changing_outfit() {
        let mut inv = inventory();
        let mut saved = super::seed_demo(&mut inv, id(1));
        saved.active.as_mut().expect("active").exclusive.remove(&id(714));
        let (change, _, active) = plan(&inv, appearance::Action::Wear(id(705), false), &HashMap::new(), &saved).expect("wear clothes");
        let active = active.expect("updated active base");
        assert!(active.exclusive.contains(&id(714)));
        appearance::demo_mutate(&mut inv, id(1), &change).expect("apply clothes");
        saved.active = Some(active);
        let (swap, _, _) = plan(&inv, appearance::Action::WearBase(id(703)), &HashMap::new(), &saved).expect("swap base");
        assert!(!swap.links.iter().any(|link| !link.folder && link.target == id(714)));
    }

    #[test]
    fn active_base_alone_does_not_make_a_clothing_outfit_dirty() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, _, _) = plan(&inv, appearance::Action::Wear(id(705), false), &HashMap::new(), &saved).expect("wear clothes");
        appearance::demo_mutate(&mut inv, id(1), &change).expect("apply clothes");
        assert!(!dirty(&inv, saved.active.as_ref()));
    }

    #[test]
    fn taking_off_shared_clothing_makes_the_shared_item_base_only() {
        let mut inv = inventory();
        let mut saved = super::seed_demo(&mut inv, id(1));
        let mut clothes = folder_links(&inv, id(705));
        clothes.push(appearance::item_link(&inv.items[&id(714)]));
        demo_replace(&mut inv, id(1), id(705), &clothes);
        saved.active.as_mut().expect("active").exclusive.remove(&id(714));
        let (change, _, active) =
            plan(&inv, appearance::Action::RemoveOutfit(id(705)), &HashMap::new(), &saved).expect("take off clothing");
        let active = active.expect("updated base");
        assert!(active.exclusive.contains(&id(714)));
        assert!(change.links.iter().any(|link| !link.folder && link.target == id(714)));
        appearance::demo_mutate(&mut inv, id(1), &change).expect("apply");
        saved.active = Some(active);
        let (change, _, _) = plan(&inv, appearance::Action::WearBase(id(703)), &HashMap::new(), &saved).expect("swap base");
        assert!(!change.links.iter().any(|link| !link.folder && link.target == id(714)));
    }

    #[test]
    fn taking_off_clothing_does_not_remove_items_of_the_active_base() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, _, _) = plan(&inv, appearance::Action::RemoveOutfit(id(705)), &HashMap::new(), &saved).expect("take off clothes");
        let targets: HashSet<_> = change.links.iter().map(|link| link.target).collect();
        for item in [710, 711, 712, 713, 714] {
            assert!(targets.contains(&id(item)), "missing base item {item}");
        }
        assert!(!targets.contains(&id(715)));
    }

    #[test]
    fn saved_folder_links_work_with_an_empty_local_base_catalog() {
        let mut inv = inventory();
        super::seed_demo(&mut inv, id(1));
        let saved = Saved::default();
        let unlinked = set_link(&inv, id(704), None, &saved.folders).expect("remove discovered link");
        assert!(!unlinked.links.iter().any(|link| link.folder && link.target == id(702)));
        let (change, _, active) =
            plan(&inv, appearance::Action::Wear(id(704), false), &HashMap::new(), &saved).expect("wear linked outfit");
        let active = active.expect("discovered bases");
        assert!(active.contains(id(703)) && active.contains(id(750)));
        assert!(change.links.iter().any(|link| !link.folder && link.target == id(730)));
        assert!(change.links.iter().any(|link| !link.folder && link.target == id(751)));
    }

    #[test]
    fn linking_a_base_rejects_a_path_back_to_the_outfit() {
        let mut cyclic = inventory();
        super::seed_demo(&mut cyclic, id(1));
        let bases = folders(&cyclic);
        add_link(&mut cyclic, id(702), id(704));
        assert!(matches!(
            set_link(&cyclic, id(704), Some(id(702)), &bases),
            Err(ResolveError::Cycle)
        ));

        let mut inv = inventory();
        super::seed_demo(&mut inv, id(1));
        let bases = folders(&inv);
        let link = set_link(&inv, id(704), Some(id(702)), &bases).expect("link base");
        assert!(link.links.iter().any(|link| link.folder && link.target == id(702)));
        demo_replace(&mut inv, id(1), id(704), &link.links);
        add_link(&mut inv, id(702), id(704));
        let remove = set_link(&inv, id(704), Some(id(702)), &bases).expect("remove cyclic link");
        assert!(!remove.links.iter().any(|link| link.folder && link.target == id(702)));
    }

    #[test]
    fn adding_or_removing_one_link_keeps_the_other_base_links() {
        let mut inv = inventory();
        super::seed_demo(&mut inv, id(1));
        let bases = folders(&inv);
        let added = set_link(&inv, id(704), Some(id(702)), &bases).expect("add body base");
        demo_replace(&mut inv, id(1), id(704), &added.links);
        let linked: HashSet<_> = linked_bases(&inv, id(704), &bases).into_iter().collect();
        assert_eq!(linked, HashSet::from([id(702), id(703), id(750)]));
        let removed = set_link(&inv, id(704), Some(id(702)), &bases).expect("remove body base");
        demo_replace(&mut inv, id(1), id(704), &removed.links);
        assert_eq!(linked_bases(&inv, id(704), &bases), vec![id(703), id(750)]);
    }

    #[test]
    fn a_cycle_in_any_linked_base_refuses_the_whole_outfit_and_graph_walk_terminates() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        add_link(&mut inv, id(750), id(704));
        assert_eq!(resolve(&inv, id(750)), Err(ResolveError::Cycle));
        assert!(plan(&inv, appearance::Action::Wear(id(704), false), &HashMap::new(), &saved).is_err());
        let (folders, _) = graph(&inv, &[id(750)]);
        assert!(folders.len() <= inv.folders.len());
    }

    #[test]
    fn saving_clothes_keeps_the_active_base_as_a_folder_link() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, _, _) = plan(&inv, appearance::Action::Save(Some("New clothes".into())), &HashMap::new(), &saved).expect("save");
        assert!(change.links.iter().any(|link| link.folder && link.target == id(702)));
        assert!(!change.links.iter().any(|link| !link.folder && link.target == id(714)));
        assert!(
            change
                .cof
                .as_ref()
                .is_some_and(|(_, links)| links.iter().any(|link| !link.folder && link.target == id(714)))
        );
    }

    #[test]
    fn saving_to_a_base_replaces_its_items_without_switching_the_current_outfit() {
        let mut inv = inventory();
        let saved = super::seed_demo(&mut inv, id(1));
        let (change, sync, _) = plan(&inv, appearance::Action::SaveTo(id(702)), &HashMap::new(), &saved).expect("save to base");

        assert_eq!(change.folder, id(702));
        assert!(!sync);
        assert!(
            change.cof.is_none(),
            "saving into a base must leave the Current Outfit folder unchanged"
        );
        assert!(!change.links.is_empty(), "the base receives the currently worn items");
        assert!(
            change.links.iter().all(|link| !link.folder),
            "a base save cannot create folder cycles"
        );
    }

    #[test]
    fn rejects_direct_and_indirect_cycles() {
        let mut inv = inventory();
        add_link(&mut inv, id(702), id(702));
        assert_eq!(resolve(&inv, id(702)), Err(ResolveError::Cycle));
        let mut inv = inventory();
        add_link(&mut inv, id(702), id(703));
        add_link(&mut inv, id(703), id(704));
        add_link(&mut inv, id(704), id(702));
        assert_eq!(resolve(&inv, id(702)), Err(ResolveError::Cycle));
    }

    #[test]
    fn shared_descendants_are_not_cycles_and_items_are_unique() {
        let mut inv = inventory();
        add_link(&mut inv, id(702), id(703));
        add_link(&mut inv, id(702), id(704));
        add_link(&mut inv, id(703), id(705));
        add_link(&mut inv, id(704), id(705));
        let links = resolve(&inv, id(702)).expect("acyclic shared base");
        let unique: HashSet<_> = links.iter().map(|link| link.target).collect();
        assert_eq!(links.len(), unique.len());
        assert!(!links.is_empty());
    }

    #[test]
    fn clothing_replacement_preserves_only_the_active_base_items_still_worn() {
        let inv = inventory();
        let current = folder_links(&inv, id(701));
        let active = Active {
            folder: id(702),
            members: HashMap::from([(
                id(702),
                BaseItems {
                    items: [id(714), id(717), id(723)].into_iter().collect(),
                    exclusive: HashSet::new(),
                },
            )]),
            items: [id(714), id(717), id(723)].into_iter().collect(),
            exclusive: HashSet::new(),
        };
        let mut next = vec![appearance::item_link(&inv.items[&id(722)])];
        preserve(&inv, &current, &mut next, &active);
        let targets: HashSet<_> = next.iter().map(|link| link.target).collect();
        assert_eq!(targets, [id(714), id(717), id(722)].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn base_swap_keeps_clothing_and_replaces_only_exclusive_items() {
        let mut inv = inventory();
        let current = folder_links(&inv, id(701));
        let old = Active {
            folder: id(702),
            members: HashMap::from([(
                id(702),
                BaseItems {
                    items: [id(711), id(714), id(717), id(718)].into_iter().collect(),
                    exclusive: [id(711), id(714), id(717), id(718)].into_iter().collect(),
                },
            )]),
            items: [id(711), id(714), id(717), id(718)].into_iter().collect(),
            exclusive: [id(711), id(714), id(717), id(718)].into_iter().collect(),
        };
        let mut skin = inv.items[&id(711)].clone();
        skin.id = id(900);
        inv.add_items(vec![skin]);
        let incoming = vec![
            appearance::item_link(&inv.items[&id(900)]),
            appearance::item_link(&inv.items[&id(723)]),
        ];
        let clothing = [id(714), id(715)].into_iter().collect();
        let (next, active) = swap(&inv, &current, Some(&old), id(703), &incoming, &clothing);
        let targets: HashSet<_> = next.iter().filter(|link| !link.folder).map(|link| link.target).collect();
        for retained in [710, 712, 713, 714, 715, 716, 719, 720, 721, 723, 900] {
            assert!(targets.contains(&id(retained)), "expected {retained}");
        }
        for removed in [711, 717, 718] {
            assert!(!targets.contains(&id(removed)), "unexpected {removed}");
        }
        assert_eq!(active.items, [id(900), id(723)].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn items_shared_with_the_clothing_outfit_are_not_exclusive_to_the_base() {
        let inv = inventory();
        let current = folder_links(&inv, id(701));
        let incoming = vec![
            appearance::item_link(&inv.items[&id(714)]),
            appearance::item_link(&inv.items[&id(723)]),
        ];
        let (_, active) = swap(&inv, &current, None, id(702), &incoming, &[id(714)].into_iter().collect());
        assert_eq!(active.exclusive, [id(723)].into_iter().collect::<HashSet<_>>());
    }

    #[test]
    fn refuses_missing_and_unloaded_folders() {
        let mut inv = inventory();
        add_link(&mut inv, id(702), id(9000));
        assert_eq!(resolve(&inv, id(702)), Err(ResolveError::Missing));
        inv.folders.get_mut(&id(702)).expect("outfit").state = super::super::inventory::FetchState::Unknown;
        assert_eq!(resolve(&inv, id(702)), Err(ResolveError::Loading));
    }
}
