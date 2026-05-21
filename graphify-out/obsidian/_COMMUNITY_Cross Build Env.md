---
type: community
cohesion: 0.22
members: 9
---

# Cross Build Env

**Cohesion:** 0.22 - loosely connected
**Members:** 9 nodes

## Members
- [[CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER]] - code - cross/jetson-orin/build.sh
- [[CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS]] - code - cross/jetson-orin/build.sh
- [[CC_aarch64_unknown_linux_gnu]] - code - cross/jetson-orin/build.sh
- [[PKG_CONFIG_ALLOW_CROSS]] - code - cross/jetson-orin/build.sh
- [[PKG_CONFIG_DIR]] - code - cross/jetson-orin/build.sh
- [[PKG_CONFIG_LIBDIR]] - code - cross/jetson-orin/build.sh
- [[PKG_CONFIG_PATH]] - code - cross/jetson-orin/build.sh
- [[PKG_CONFIG_SYSROOT_DIR]] - code - cross/jetson-orin/build.sh
- [[build.sh]] - code - cross/jetson-orin/build.sh

## Live Query (requires Dataview plugin)

```dataview
TABLE source_file, type FROM #community/Cross_Build_Env
SORT file.name ASC
```
