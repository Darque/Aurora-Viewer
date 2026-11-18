# Outfit variants in Appearance

This sandbox supports outfit variants as plain child folders under an outfit
in **My Outfits**. The parent folder holds items shared by every variant; each
child folder is one selectable variation. A child must be a plain inventory
folder directly under the outfit.

The Appearance preview shows left and right controls when variants are
available. When there is one child folder, the selector also includes
**Main outfit**, so that accessory can be worn with the direct outfit items or
left off. With multiple variants, one of the variants is always selected. Use **Add outfit
variant** in the outfit preview or thumbnail menu to create one; new variants
are named `Variant`, `Variant 2`, and so on. The selected child supplies the
outfit's thumbnail and detailed portrait, and the selection is remembered in
the viewer's settings. Wearing the outfit applies its direct items together
with the selected variant's contents; sibling variants are excluded. The same
behavior applies to base outfits.

When an outfit is expanded in the **Outfits** or **Base outfits** list, its
variants appear at the same indent as the outfit's items, with their contents
shown beneath each folder. Select a row to make that variation active, or drag
wearable inventory items onto it to add links to that variation. Dragging a
link from another outfit moves that link into the variation; dragging an
ordinary inventory item leaves the original in place and adds an outfit link.
The variant folder is also available in Inventory, where it accepts wearable
items through the same outfit-link rules.

This is experimental sandbox behavior and may change as the viewer source
advances.
