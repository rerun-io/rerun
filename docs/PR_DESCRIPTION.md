# PR Description

## Summary

This PR consolidates and significantly improves the blueprint documentation across the docs. The changes reorganize blueprint-related content into clearer, more comprehensive guides while updating all internal references.

## Key Changes

### New Documentation

- **`content/concepts/blueprints.md`** - Complete rewrite of the blueprints concept page with:
  - Clearer explanation of what blueprints are and how they work
  - Detailed coverage of Application ID binding behavior
  - Comprehensive explanation of reset behaviors (heuristic vs default)
  - Better organized sections on blueprint architecture and use cases
  - Embedded video tutorial
  - Visual diagrams and screenshots throughout

- **`content/reference/viewer/blueprints.md`** - New comprehensive blueprint UI reference covering:
  - Detailed explanation of all blueprint panel features
  - Controls, context menus, and shortcuts
  - View hierarchy management
  - Entity queries and filtering
  - Overrides and visualizer configuration

- **`content/getting-started/configure-the-viewer.md`** - Massively expanded from a brief overview to a complete tutorial covering:
  - Interactive configuration with step-by-step screenshots
  - Save and load blueprint files workflow
  - Programmatic blueprints with full code examples
  - Practical examples for all three approaches
  - Complete cross-references to related documentation

### Removed Documentation

- **`content/concepts/blueprint.md`** - Replaced by the improved `blueprints.md`
- **`content/reference/viewer/blueprint.md`** - Replaced by the improved `blueprints.md`

### Link Updates

Updated all internal references from old blueprint documentation paths to the new structure:
- `content/concepts/apps-and-recordings.md`
- `content/concepts/entity-path.md`
- `content/concepts/visualizers-and-overrides.md`
- Multiple references in `content/reference/dataframes.md`
- References in `content/reference/viewer/overview.md`

## Documentation Improvements

### Better Organization
- Clear separation between concepts (what blueprints are), getting started (how to use them), and reference (UI details)
- More logical progression from interactive → file-based → programmatic workflows
- Comprehensive cross-linking between related topics

### Enhanced Content
- More detailed explanations with practical examples
- Extensive use of screenshots and visual aids
- Code examples for programmatic blueprint usage
- Real-world use cases and patterns
- Clearer explanation of Application ID behavior and its implications

### Improved Accessibility
- Step-by-step tutorials with screenshots for each action
- Video tutorial embedded for visual learners
- Better section headers and navigation
- More actionable "Next steps" sections

## Impact

This PR provides users with:
- A clearer mental model of how blueprints work
- Practical guidance for all three blueprint workflows
- Better discoverability of blueprint features
- Reduced confusion around Application IDs and reset behavior
- More comprehensive reference material for blueprint UI features
