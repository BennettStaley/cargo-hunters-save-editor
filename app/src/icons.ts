// Resolve an item to the best existing `Icon_*` sprite, ported from the Python
// `resolve_icon`. Returns a sprite stem; the UI loads `/sprites/<stem>.png`.

const WEAPON_CLASS: Record<string, string> = {
  AssaultRifles: "Icon_AR",
  Rifles: "Icon_AR",
  SniperRifles: "Icon_SR",
  MarksmanRifles: "Icon_HSR",
  Shotguns: "Icon_Shotgun",
  SMG: "Icon_SMG",
  Pistols: "Icon_Pistol",
  Machineguns: "Icon_LMG",
  HeavyGuns: "Icon_HMG",
  Melee: "Icon_Melee",
  Grenade: "Icon_Grenades",
  Modules: "Icon_WeaponParts",
};

const LOOT_KEYWORDS: [string[], string][] = [
  [["gold", "jewel", "ring", "watch", "bullion", "emperor", "diamond", "silver", "vintage"], "Icon_Valuables"],
  [["cpu", "gpu", "ssd", "hdd", "radio", "phone", "laptop", "circuit", "sensor", "battery",
    "powersupply", "power_supply", "powerbank", "motor", "relay", "chip", "microcont",
    "microcircuit", "electron", "gamepad", "console", "oscillo", "multimeter", "tablet",
    "gyroscop", "datameter", "scaner", "scanner", "camera", "monitor", "boombox", "hifi",
    "vacuumtube", "vacuum_tube", "jumpstarter", "carbattery"], "Icon_Devices"],
  [["wrench", "screwdriver", "saw", "hammer", "drill", "plier", "cutter", "grinder",
    "welding", "soldering", "knife", "scissor", "vise", "shovel", "perforator", "honing",
    "multitool", "polisher", "spanner", "shears", "heatgun", "gascutter", "bending",
    "hacksaw", "utilityknife", "wirecutter", "setoftools", "setofwrenches"], "Icon_Tools"],
  [["tnt", "anfo", "torpex", "gunpowder", "explos", "termite", "detonator", "fuse"], "Icon_ExplosivesParts"],
  [["repairkit", "medkit", "aid", "bandage", "syringe", "pills", "injector", "medicine", "anticor"], "Icon_Aid_Kits"],
  [["grinder_disk", "angle_grinder", "angledisk", "disk"], "Icon_GrinderDiscs"],
  [["tarpaulin", "fabric", "thread", "textile", "cloth", "tshirt", "tent", "rags",
    "sewing", "fiber", "tracksuit", "sneaker", "towrope", "clothline", "trousers", "hammock"], "Icon_Textile"],
  [["fuel", "petrol", "gas", "canister", "solvent", "epoxy", "glue", "sealant", "reagent",
    "chemical", "foam", "oilfilter", "paint", "coating"], "Icon_Fuel"],
  [["metal", "steel", "aluminum", "alum", "brake", "bolt", "nut", "screw", "pipe", "cable",
    "wire", "plate", "tube", "coil", "bearing", "granulate", "powder", "reinforced",
    "sparkplug", "fitting", "nail", "structure", "hydraulics", "titanium", "gunparts",
    "armorplate", "metalparts", "metaltube"], "Icon_Materials"],
  [["key", "keycard"], "Icon_Keys"],
];

// The set of sprite stems that actually exist (copied into public/sprites).
// Loaded lazily; if a resolved name is missing we still return it (the <img>
// onerror falls back to a neutral icon).
export function resolveIcon(visualName: string, itemName: string): string {
  const parts = (visualName || "").replace(".prefab", "").split("/").filter(Boolean);
  const top = parts[0] ?? "";
  const sub = parts[1] ?? "";
  const leaf = (parts[parts.length - 1] ?? "").toLowerCase();
  const hay = `${leaf} ${(itemName || "").toLowerCase()}`;
  const has = (k: string) => hay.includes(k);

  if (top === "Weapons") return WEAPON_CLASS[sub] ?? "Icon_Weapons";
  if (
    parts.includes("Modules") ||
    (top === "Items" && sub === "Weapons") ||
    ["barrel", "receiver", "buttstock", "handguard", "suppressor", "muzzle", "foregrip",
      "picatinny", "rifle stock", "_stock"].some(has)
  )
    return "Icon_WeaponParts";
  if (top === "Ammo") return "Icon_Ammo";
  if (top === "BodyParts") {
    if (sub.includes("Head")) return "Icon_Head";
    if (sub.includes("Arm")) return "Icon_Arms";
    if (sub.includes("Leg")) return "Icon_Legs";
    if (sub.includes("Torso")) return "Icon_Torso";
    return "Icon_Bodyparts";
  }
  if (top === "Outfits") {
    if (sub.includes("Helmet") || sub.includes("Hat")) return "Icon_Headgear";
    if (sub.includes("Backpack")) return "Icon_Backpacks";
    if (sub.includes("Vest") || sub.includes("Armor")) return "Icon_PlateCarriers";
    return "Icon_Equipment";
  }
  if (top === "Keys" || sub === "Keys") return "Icon_Keys";
  if (top === "Droid" || sub === "Droid") return "Icon_AnDComponents";
  if (leaf.includes("case") || leaf.includes("safe") || sub === "Cases" || top === "LootContainers")
    return "Icon_Cases";
  if (top === "Tools" || sub === "Tools") return "Icon_Tools";
  for (const [kws, icon] of LOOT_KEYWORDS) if (kws.some(has)) return icon;
  return "Icon_Surplus";
}

export function iconUrl(stem: string): string {
  return `/sprites/${stem}.png`;
}
