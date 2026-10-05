---
category: maint
breaking: false
authors: [dotdat]
---

Test that an unconfigured user is unaffected

New integration tests check that, with no configuration, `rover config show` and other commands create nothing beyond the files Rover already wrote before profile settings existed. They also check that a read-only configuration directory doesn't make those commands fail. The smoke tests now run the end-to-end suite with Rover's config home pointed at an empty location.
