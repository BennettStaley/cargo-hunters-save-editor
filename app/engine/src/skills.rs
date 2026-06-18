//! Skill metadata, baked from the game's `skills` TextAsset. The save stores
//! skills by numeric `Id` only (with no names); this maps each Id to its
//! display name, icon, max level, disabled flag, and the game's display order
//! so the UI can show real, ordered, named skills.
//!
//! Source: CargoHunters_Data/.../repositoriesgroup_assets_all_*.bundle ->
//! TextAsset "skills". The `Disabled__*` entries are deprecated handling skills
//! the game keeps for back-compat. Names are humanized from each skill's Alias.

pub struct SkillMeta {
    pub name: &'static str,
    pub icon: &'static str,
    pub max_level: i64,
    pub disabled: bool,
    pub order: i64,
}

/// Metadata for a skill Id, or None if the Id isn't in the known set.
pub fn skill_meta(id: i64) -> Option<SkillMeta> {
    Some(match id {
        1 => SkillMeta { name: "Pistol Handling", icon: "IconSkill_Pistol", max_level: 25, disabled: true, order: 9999 },
        2 => SkillMeta { name: "Submachine Gun Handling", icon: "IconSkill_SubmachineGun", max_level: 25, disabled: true, order: 9999 },
        3 => SkillMeta { name: "Assault Rifle Handling", icon: "IconSkill_AssaultRifle", max_level: 25, disabled: true, order: 9999 },
        4 => SkillMeta { name: "Shotgun Handling", icon: "IconSkill_Shothun", max_level: 25, disabled: true, order: 9999 },
        5 => SkillMeta { name: "Sniper Rifle Handling", icon: "IconSkill_SniperRifle", max_level: 25, disabled: true, order: 9999 },
        6 => SkillMeta { name: "Marksman Rifle Handling", icon: "IconSkill_Marksman", max_level: 25, disabled: true, order: 9999 },
        7 => SkillMeta { name: "Machine Gun Handling", icon: "IconSkill_MachineGun", max_level: 25, disabled: true, order: 9999 },
        9 => SkillMeta { name: "Throwing", icon: "IconSkill_Grenade", max_level: 25, disabled: true, order: 9999 },
        10 => SkillMeta { name: "Item Find", icon: "IconSkill_Exploring", max_level: 6, disabled: false, order: 2 },
        11 => SkillMeta { name: "First Aid", icon: "IconSkill_Repair", max_level: 6, disabled: false, order: 3 },
        12 => SkillMeta { name: "Combat", icon: "IconSkill_Combat", max_level: 6, disabled: false, order: 4 },
        13 => SkillMeta { name: "Mobility", icon: "IconSkill_Mobility", max_level: 6, disabled: false, order: 8 },
        14 => SkillMeta { name: "Melee Handling", icon: "IconSkill_MeleeGun", max_level: 25, disabled: true, order: 9999 },
        16 => SkillMeta { name: "Backpacking", icon: "IconSkill_Backpacking", max_level: 6, disabled: false, order: 1 },
        17 => SkillMeta { name: "Angle Grinder Handling", icon: "IconSkill_AngleGrinder", max_level: 25, disabled: true, order: 9999 },
        18 => SkillMeta { name: "Pistol Handling", icon: "IconSkill_Pistol", max_level: 3, disabled: false, order: 10 },
        19 => SkillMeta { name: "Assault Rifle Handling", icon: "IconSkill_AssaultRifle", max_level: 3, disabled: false, order: 13 },
        20 => SkillMeta { name: "Shotgun Handling", icon: "IconSkill_Shothun", max_level: 3, disabled: false, order: 12 },
        21 => SkillMeta { name: "Sniper Rifle Handling", icon: "IconSkill_SniperRifle", max_level: 3, disabled: false, order: 14 },
        22 => SkillMeta { name: "Marksman Rifle Handling", icon: "IconSkill_Marksman", max_level: 3, disabled: false, order: 15 },
        23 => SkillMeta { name: "SMG Handling", icon: "IconSkill_SubmachineGun", max_level: 3, disabled: false, order: 11 },
        24 => SkillMeta { name: "Melee Handling", icon: "IconSkill_MeleeGun", max_level: 3, disabled: false, order: 16 },
        25 => SkillMeta { name: "Secondary Weapon", icon: "IconSkill_CarryTwoPrimary", max_level: 3, disabled: false, order: 5 },
        26 => SkillMeta { name: "Sound Locator", icon: "IconSkill_BuiltInSonar", max_level: 3, disabled: false, order: 6 },
        27 => SkillMeta { name: "Lockpick Handling", icon: "IconSkill_Lockpicking", max_level: 5, disabled: false, order: 7 },
        _ => return None,
    })
}
