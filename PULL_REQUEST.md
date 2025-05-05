# Make start_point_file configurable in project creation output

## What Changed
This PR enhances the project creation output to use a configurable start point file instead of hardcoding "getting-started.md". The changes allow templates to specify their own starting point file while maintaining backward compatibility.

### Key Changes:
1. Updated `generate_project_created_message` to accept a `start_point_file` parameter
2. Modified `display_project_created_message` to pass through the template's start point file
3. Updated the `ProjectCreated` state to use the template's `start_point_file` when available
4. Enhanced test coverage to verify both default and custom start point file scenarios

### Code Changes:
- `src/command/init/helpers.rs`: Added `start_point_file` parameter to message generation functions
- `src/command/init/transitions.rs`: Updated `ProjectCreated` state to pass template's start point file
- `src/command/init/tests/project_created_message_tests.rs`: Added new test cases and updated existing ones

### Example Output
Before:
```
For more information, check out 'getting-started.md'.
```

After (with custom template):
```
For more information, check out 'readme.md'.
```

## Testing
- Added new test case `test_display_project_created_message_with_custom_start_point`
- Updated existing test cases to verify start point file behavior
- Verified both default ("getting-started.md") and custom ("readme.md") scenarios
- All tests pass with proper message formatting

## Backwards Compatibility
- Maintains default behavior of using "getting-started.md" when no template is specified
- Uses feature flags to handle both "init" and non-"init" cases
- No breaking changes to existing functionality

## Related Issues
Closes #[ISSUE_NUMBER] <!-- Replace with actual issue number -->

## Checklist
- [x] Tests added/updated
- [x] Documentation updated
- [x] Feature flags properly handled
- [x] Backwards compatibility maintained
- [x] Error cases handled
- [x] Code reviewed 