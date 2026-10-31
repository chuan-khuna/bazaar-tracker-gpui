# UnitySpy: Process access

This doc covers how UnitySpy reads another process's memory and finds loaded modules: the `ProcessFacade` class hierarchy in `src/HackF5.UnitySpy/ProcessFacade/` and the native helper in `src/HackF5.UnitySpy.Native/`. Read [overview.md](overview.md) for context.

How each claim is marked:
- **[V]**: verified from source.
- **[I]**: inferred.

## 1. Class hierarchy [V]

```
ProcessFacade (abstract)                      ProcessFacade.cs
├── ProcessFacadeWindows                      ReadProcessMemory + psapi
├── ProcessFacadeLinux (abstract)             /proc/<pid>/maps parsing
│   ├── ProcessFacadeLinuxDirect              /proc/<pid>/mem via FileStream
│   ├── ProcessFacadeLinuxPTrace              process_vm_readv
│   ├── ProcessFacadeLinuxDump                dumped region files on disk
│   └── ProcessFacadeLinuxClient              TCP to server.c
└── ProcessFacadeMacOS (abstract)             holds a Process
    ├── ProcessFacadeMacOSDirect              P/Invoke into macos.dylib
    └── ProcessFacadeMacOSClient              TCP to server.c

UnityProcessFacade                            wraps ProcessFacade + MonoLibraryOffsets
ProcessFacadeClient                           the TCP client used by both *Client facades
```

A subclass implements only two things: `ReadProcessMemory(byte[] buffer, IntPtr address, bool allowPartialRead = false, int? size = null)` and `GetModule(string name) → ModuleInfo`. `ModuleInfo` holds `ModuleName`, `BaseAddress`, `Size` (uint) and `Path` (`ModuleInfo.cs:9-23`). [V]

## 2. The `ProcessFacade` base [V]

- `Is64Bits` is a settable property, and `SizeOfPtr` is 8 or 4 (`ProcessFacade.cs:17-19`). The facade never detects bitness itself. `UnityProcessFacade`'s constructor copies it from the offset table (`UnityProcessFacade.cs:19-28`). Until then it is `false`, so pointer reads would be 4 bytes wide.
- Typed reads (`ReadInt32`, `ReadUInt32`, `ReadInt64`, `ReadUInt64`, `ReadByte`, `ReadPtr`) all go through `ReadBufferValue(address, size, convert)` (`ProcessFacade.cs:177-190`):
  - It rents a buffer from `ByteArrayPool`.
  - It calls `ReadProcessMemory(buffer, address, size: size)`. Partial reads are never allowed here.
  - It converts with `BitConverter`, which is little-endian on every supported host.
- `ReadPtr` reads a u64 or u32 depending on `Is64Bits` and casts it to `IntPtr` (`ProcessFacade.cs:145-146`).
- `ReadAsciiString(address, maxSize = 1024)` reads **exactly** `maxSize` bytes in one call and cuts at the first NUL (`ProcessFacade.cs:21-22`, `Util/ConversionUtils.cs:9-13`). Class names, namespaces, field names and assembly names are all read this way. If the string sits within 1,024 bytes of the end of a readable region, the read fails with an exception rather than returning a short string. [V for the code; I that this happens in practice]
- `ReadModule(moduleInfo)` allocates `moduleInfo.Size` bytes and reads the whole module image in a single call (`ProcessFacade.cs:157-167`). It is used only to parse the PE export table.

### 2.1 `ByteArrayPool` [V]

`Util/ByteArrayPool.cs` is a lock-free pool adapted from Roslyn's `ObjectPool`:
- It holds up to 16 buffers of **16 bytes** each, plus a fast-path first slot.
- A request larger than 16 bytes always allocates a new array and is never pooled.
- `Return` zeroes the buffer before pooling it.
- Because the rented buffer is always 16 bytes, a converter that reads more bytes than were actually fetched sees zeros, not stale data. This is why the float bug in [type-model.md](type-model.md) §6 returns wrong values instead of throwing.

## 3. Windows: `ProcessFacadeWindows` [V]

- **Constructor**: it takes a `System.Diagnostics.Process` and keeps it (`ProcessFacadeWindows.cs:23-27`).
- **Handle**: every call uses `this.process.Handle`. There is no explicit `OpenProcess` call in the library. .NET's `Process.Handle` opens the process with `PROCESS_ALL_ACCESS` [I: .NET runtime behaviour, not in this tree]. That fails against a process at a higher integrity level, which may be why the BazaarDBTracker fork switched to `OpenProcess(0x410)` and also relaunches itself as admin.
- **`ReadProcessMemory`** (`ProcessFacadeWindows.cs:31-67`):
  1. It pins the buffer.
  2. It calls `kernel32!ReadProcessMemory(handle, address, buf, size ?? buffer.Length, out _)`.
  3. On failure it reads `GetLastWin32Error`. If the error is `299` (`ERROR_PARTIAL_COPY`) **and** `allowPartialRead` is set, it returns silently. Otherwise it throws `Win32Exception`.
  4. The number of bytes actually read is ignored.
- **`GetMainModuleFileName`** uses `QueryFullProcessImageName` with a 1,024-character buffer (`:69-76`). The GUIs feed this into `MonoLibraryOffsets.GetOffsets`.
- **`GetModule(name)`** (`:88-121`):
  1. `EnumProcessModulesEx(handle, IntPtr[2048 * SizeOfPtr], …, 0x03 = LIST_MODULES_ALL)` (`:159-179`).
  2. For each module, `GetModuleFileNameEx` (it throws `COMException` if that returns 0) and `GetModuleInformation`, giving `BaseOfDll` and `SizeInBytes`.
  3. It returns the first module whose file name equals `name` exactly, with a case-sensitive `==`. If nothing matches it returns **`null`**. The caller (`AssemblyImageFactory`) then fails with an `ArgumentNullException` in `ReadModule`, not a clear "Mono not found" message.
  4. Quirk: the `cb` argument of `EnumProcessModulesEx` is passed as the array's **element count**, not its byte size, and `GetModuleInformation` gets `IntPtr.Size * count` as `cb` (`:113`, `:167`). Both still work because they over- or under-state a large enough buffer. [V for the code, I for the effect]
- The comment at `:78-87` says the managed `Process.Modules` API would only work when UnitySpy's own bitness matches the target's. The Avalonia GUI is built x64 only (`Gui.Avalonia.csproj:6`).

## 4. Linux: `ProcessFacadeLinux` and subclasses [V]

### 4.1 Module map

`ProcessFacadeLinux` (`ProcessFacadeLinux.cs`) parses a `maps` file. The default is `/proc/<pid>/maps`; the dump mode passes an explicit path.
- Each line is split on spaces. The range comes from column 0.
- The **name is taken as `line.Substring(73)`**, a fixed column offset (`:33-36`). This assumes the kernel's padding is always the same width, which breaks on lines with longer inode numbers or other padding [I].
- `IsStartingModule` is set when column 4 (the inode) is not `"0"` (`:42`).
- `GetModule(name)` finds the first mapping whose name ends with `name`, then extends over the following mappings until the next file-backed mapping with a different name. The size is `end − start` of that span (`:72-93`).
- `ReadProcessMemory` checks that the start address lies in some mapping. If it does not, it **fills the buffer with zeros and logs to stderr instead of failing** (`:46-70`). A wild pointer therefore reads as `0` or `null`, not as an error.

### 4.2 The four readers

| Class | Mechanism | Privilege | Notes |
|---|---|---|---|
| `ProcessFacadeLinuxDirect` | Opens a new `FileStream` on `/proc/<pid>/mem` **for every read**, seeks, and reads (`ProcessFacadeLinuxDirect.cs:22-35`). | Needs ptrace-level read access to the target (root, or `ptrace_scope=0` and the same user) [I]. | One open per field read is slow. |
| `ProcessFacadeLinuxPTrace` | `libc!process_vm_readv` with one local and one remote iovec (`ProcessFacadeLinuxPTrace.cs:23-62`). | Same as above [I]. | Despite the name, it never calls `ptrace`. It also copies the buffer onto itself afterwards (`Marshal.Copy(ptr, buffer, …)` where `ptr` already points into `buffer`). |
| `ProcessFacadeLinuxDump` | Reads files in a directory whose names are `<startHex>-<endHex>`. It stitches reads across adjacent dump files recursively (`ProcessFacadeLinuxDump.cs:17-89`). | None. | Offline analysis of a memory dump, paired with a saved `maps` file. |
| `ProcessFacadeLinuxClient` | Forwards reads to `ProcessFacadeClient` (TCP) (`ProcessFacadeLinuxClient.cs`). | The server runs as root; the GUI runs as the user. | See §6. |

Linux has **no ELF offset table**, so these backends are only useful for a Windows (PE) Mono game under Wine/Proton. In that case the mapped `mono-2.0-bdwgc.dll` is a PE image and the PE code path applies. [V that no ELF table or ELF parser exists; I for the intended use]

## 5. macOS [V]

- **`ProcessFacadeMacOSDirect`** P/Invokes `macos.dylib` (`ProcessFacadeMacOSDirect.cs:81-93`):
  - `read_process_memory_to_buffer(pid, addr, buf, size)` returns 0 on success.
  - `get_module_info(pid, name, ref size, pathBuf)` returns the base address.
  - `GetModule` throws for 32-bit.
- **`macos.c`** implements these with `task_for_pid` and `vm_read`:
  - Modules come from `task_info(TASK_DYLD_INFO)`, then `dyld_all_image_infos`, then a suffix match on `imageFilePath`.
  - The image size is computed by summing `LC_SEGMENT_64` `vmsize`s.
  - It calls `task_for_pid` on every read and never deallocates the `vm_read` buffers, so it leaks memory [V for the code; I for the effect].
  - `task_for_pid` needs root or debugger entitlements [I].
- **`ProcessFacadeMacOSClient`** uses the TCP server for both reads and module lookup.
- **Build gap**: the `Makefile` only builds `server`. There is no target that builds `macos.dylib`, although the library csproj bundles `../macos.dylib` (`HackF5.UnitySpy.csproj:37-39`).

## 6. The native TCP server (`server.c`) and `ProcessFacadeClient` [V]

**Transport.** The server listens on TCP port **39185** on `INADDR_ANY`, so it is reachable from **all interfaces**. It spawns one pthread per connection (`server.c:17`, `:109-148`). There is **no authentication**: any host that can reach the port can read any process the server (running as root) can read [I for the risk].

**Client.** `ProcessFacadeClient` connects to `Dns.GetHostEntry(Dns.GetHostName()).AddressList[0]`, port 39185 (`ProcessFacadeClient.cs:167-179`). That is the machine's own first resolved address, not necessarily loopback.

**Request.** A request is 17 packed bytes (`ProcessFacadeClient.cs:207-235`, `server.c:21`):

| Bytes | Field |
|---|---|
| 0 | type: 0 = read memory, 1 = get module |
| 1–4 | pid (int32, LE) |
| 5–12 | address (int64, LE) |
| 13–16 | size (int32, LE) |

The client builds this by marshalling a sequential struct and copying from offset 4, which skips the padded `byte type`.

**Read memory.**
- The server reads in chunks of up to 4,096 bytes (`linux.c`: `fopen("/proc/<pid>/mem")`, `fseek` and `fread`, reopening the file for every chunk) and sends each chunk (`server.c:71-97`).
- If a chunk read returns 0 bytes, `remaining_bytes` never decreases and the loop **spins forever** (`:79-96`).
- The client loops on `Receive` until `length` bytes have arrived (`ProcessFacadeClient.cs:64-71`).
- **Every exception is caught, logged to the console and swallowed.** The connection is closed, and the caller gets a partly filled buffer with no error (`:73-93`).

**Get module.** The client sends the request (`size` = name length), then the ASCII name. The server replies with three things (`server.c:38-69`):
- the base address (8 bytes);
- `module_size` as a 4-byte **int**;
- the path bytes **with no length prefix**.

The client instead reads 8 bytes for the size and then expects a 4-byte length prefix before the path (`ProcessFacadeClient.cs:114-122`, `:145-164`). **These framings do not match**, so module lookup over the server is broken [V for both sides; I that it fails at runtime].

Other bugs in this path:
- The server NUL-terminates the name with `buff[(int)size]`, where `size` is a **pointer**, not the length (`server.c:48`).
- On Linux, `get_module_info` is a stub that returns `NULL` (`linux.c:31-34`). The Linux client never calls it, because it gets modules from the maps file.

## 7. Error model summary

| Backend | Read failure |
|---|---|
| Windows | `Win32Exception(code)`. A partial copy is tolerated only when `allowPartialRead` is set (raw memory view, module dump). [V] |
| Linux (all) | An unmapped address gives a **zero-filled buffer** and a stderr line. A mapped-but-failed read throws a plain `Exception`. [V] |
| macOS direct | `InvalidOperationException("Could not read memory …")`. [V] |
| TCP client | Swallowed and logged. The buffer is left partly filled. [V] |

The higher layers (`AssemblyImage`, `TypeDefinition`, `ReadManaged`) do not catch these exceptions, apart from the `VALUETYPE` fallback described in [type-model.md](type-model.md) §6 and the GUI view-models, which show `ERROR: …` strings. [V]

## 8. Notes for a Rust port [I]

- Implement one `MemoryReader` trait (`read_exact(addr, &mut [u8]) -> Result<()>`) with a Windows impl on `OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION)` and `ReadProcessMemory`. Treat `ERROR_PARTIAL_COPY` as an error by default. Give a dead PID or handle its own error variant, so the caller can tell to reconnect.
- Bulk-read where possible. UnitySpy issues one syscall per primitive, and attaching alone costs several syscalls per class, times every class in the cache. Reading whole `MonoClass` structs (about 0x110 bytes on x64) and whole array bodies in one call is cheap.
- Read C strings in page-safe chunks (for example up to the next 4 KiB boundary, then continue) instead of a fixed 1,024 bytes.
- Do not port the Linux, macOS or TCP-server paths for a Windows-only tracker. If a Linux/Proton build is ever wanted, `process_vm_readv` (via the `nix` crate) with proper `/proc/<pid>/maps` parsing is the clean equivalent.
