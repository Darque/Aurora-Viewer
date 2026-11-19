# Outfit variants in Appearance

This sandbox supports outfit variants as plain child folders under an outfit
in **My Outfits**. The parent folder holds items shared by every variant; each
child folder is one selectable variation. A child must be a plain inventory
folder directly under the outfit.

The Appearance preview shows left and right controls when variants are
available. When there is one child folder, the selector also includes
**Main outfit**, so that accessory can be worn with the direct outfit items or
left off. With multiple variants, one of the variants is always selected. Use
**Add outfit variant** in the outfit preview or thumbnail menu to enter a name
for a new variant; duplicate sibling names receive a number. Double-click a
variant name or use its context menu to rename it. The selected child supplies
the outfit's thumbnail and detailed portrait. If the child has no picture, it
uses the parent outfit's picture until it gets its own. The selection is
remembered in the viewer's settings. Wearing the outfit applies its direct
items together with the selected variant's contents; sibling variants are
excluded. The same behavior applies to base outfits.

Gallery cards show a variant count badge. Hovering a card replaces the badge
with its Wear action; outfits with variants have a Wear menu that lets you
choose which variant to wear. The menu stays open as the pointer moves from
the Wear button to an option, and closes after choosing an option or clicking
outside it. For a single child, the menu also offers **Main outfit**.

When an outfit is expanded in the **Outfits** or **Base outfits** list, its
variants appear at the same indent as the outfit's items, with their contents
shown beneath each folder. Select a row to make that variation active,
double-click its name to rename it, or drag wearable inventory items onto it
to add links to that variation. Double-clicking a folder or object name in
Inventory starts inline rename; right-click menus also offer Rename. Dragging
a link from another outfit moves that link into the variation; dragging an
ordinary inventory item leaves the original in place and adds an outfit link.
Creating a variant commits the name in the dialog and leaves the Inventory
tree out of rename mode. The variant folder is also available in Inventory,
where it accepts wearable items through the same outfit-link rules.

Use **New outfit** beside the gear button at the bottom of Appearance to
create an outfit directly in **My Outfits**. The name is editable before
creation; a leading period marks a reusable base outfit, and duplicate names
get a number suffix. A plain empty folder created under **My Outfits** in
Inventory also becomes an outfit when the first wearable is dropped into it.
After folders have loaded, empty groups are hidden from the Base outfits and
Outfits lists. A group with base outfits appears in Base outfits, a group with
regular outfits appears in Outfits, and a group with both appears in both.

This is experimental sandbox behavior and may change as the viewer source
advances.
