---
name: combat
description: What every combat keyword does (Burn, Poison, Shield, Haste, Freeze, Multicast, Heated, Tempo, Rage…). Load it to reason about a card's text.
---

# Combat keywords

Sources:
- The game's own keyword tooltips (`GameData.db`, table `tooltips`, version 5.0.0, the prod copy written 2026-11-05), read 2026-11-09. Where they say something, they win.
- Mobalytics, "The Bazaar Keywords Explained" (<https://mobalytics.gg/the-bazaar/guides/keywords-and-terms>), read 2026-11-09, for keywords with no tooltip text (Destroy, Heated, Chilled, Notes, Quest) and for details the tooltips leave out.

The game patches often: when a card's text disagrees with this, trust the card.

## Damage over time

- **Burn**: deals damage twice a second. Each time it deals damage it decreases by 3% (no longer by a flat 1, as older guides and the game's own "Burns" tooltip still say). Shield halves Burn damage; Heal cleanses 5% of it.
- **Poison**: deals damage every second. Shield doesn't block it; Heal cleanses 5% of it.

## Defence and sustain

- **Shield**: blocks damage. Poison ignores it, and it halves Burn damage.
- **Heal**: restores Health, up to Max Health, and cleanses 5% of Poison and Burn.
- **Regen**: restores Health equal to your Regen every second.
- **Lifesteal**: when the item deals damage, you recover that much Health.

## Speed and control

- **Haste**: the item charges twice as fast.
- **Slow**: the item charges half as fast.
- **Freeze**: a Frozen item can't activate (Mobalytics: it doesn't charge either).
- **Charge**: immediately advances an item's cooldown.
- **Flying**: Freeze and Slow on the item last half as long.

## Item effects

- **Crit Chance**: the chance for an item with a cooldown to crit, which doubles its Damage, Heal, Shield, Regen, Burn and Poison active effects.
- **Multicast**: each use, the item's effect happens that many times (Dualcast, Tricast, Quadcast).
- **Ammo**: an item with no Ammo left can't fire. Ammo refills after each fight.
- **Destroy**: a Destroyed item is fully disabled: it doesn't activate, and its passives stop.
- **Repair**: returns a Destroyed item to normal.
- **Transform**: changes the item into a new form, keeping its tier and enchantment if it can. Legendaries can't be Transformed. (Mobalytics: the new item is the same size, from your hero's pool, even when the old one was neutral or another hero's.)
- **Upgrade**: raises the item's tier, which makes it stronger and worth more.
- **Enchant**: adds a unique new effect to the item.
- **Quest**: the item gains more abilities once a condition is met, often buying a number of a kind of item or reaching a stat in combat.

## Hero mechanics

- **Heated and Chilled** (Jules): items and Board slots can be Heated or Chilled; items whose text mentions it get extra effects.
- **Notes C, D, E, F, G, A, B** (The Dragons): a slot mechanic; each Note gives 10% cooldown reduction to its type of item.
- **Tempo** (The Dragons): you gain 1 Tempo per second of combat. Items and skills spend it for bonus effects; some items make more Tempo, or count the Tempo gained over the whole fight.
- **Rage** (Karnok, from the Karnok's Rage skill he starts every Run with): using items builds Rage. At 100 he becomes Enraged: Slow and Freeze are removed from his items, and while Enraged their cooldowns are 10% shorter (Mobalytics: for 5 seconds).
