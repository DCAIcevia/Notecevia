# IPC 命令

## 范围

- 前端 `crates/ui` 通过 Tauri 的 `invoke` 调窗口壳 `crates/app` 注册的命令；命令层在 `crates/app/src/ipc/`，注册表在 `crates/app/src/lib.rs` 的 `generate_handler!`。
- 命令层只做参数转换、调用 `core`／`store`、把错误换成 `IpcError`；业务规则不写在这一层。
- 依赖单向：`crates/ui` → `crates/app` → `core`、`store`；`store` → `core`。
- 注册表共 40 条命令。
- 参数名按 Tauri 约定用 camelCase（`boardId`、`fileName`），与 Rust 侧的 snake_case 对应。
- 时间属于应用边界：存盘与回收站倒计时用的 Unix 秒由命令层取，再传给 `store`。

## 工作区

- 启动时不自动打开工作区。前端先调 `app_info` 取默认路径，再调 `open_workspace`。
- `open_workspace` 省略 `path` 时用程序目录下的 `NoteceviaData`；成功返回 `WorkspaceStatus` 并替换当前工作区。
- 打开失败返回错误，已经打开的工作区保持不变。
- 工作区没打开时，除 `app_info`、`open_workspace`、`workspace_status` 外的命令返回 `workspaceNotOpen`；`workspace_status` 返回 `null`。
- 全局只持有一个工作区，放在 `Mutex` 里（`Workspace` 的索引缓存不是 `Sync`）。

```json
{ "root": "D:\\Notecevia\\NoteceviaData", "purgedOnOpen": 0 }
```

## 应用状态

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `app_info` | 无 | `AppInfo` |
| `open_workspace` | `path?: string` | `WorkspaceStatus` |
| `workspace_status` | 无 | `WorkspaceStatus` 或 `null` |

## 画板与搜索

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `list_boards` | 无 | `BoardSummary[]`，按更新时间从新到旧 |
| `search_boards` | `query: string` | `SearchHitDto[]`，同样从新到旧 |
| `rebuild_index` | 无 | `number`，重建后的画板条数 |
| `list_media` | 无 | `MediaSummary[]` |
| `next_board_id` | 无 | `number`，下一个可用画板编号 |
| `open_board` | `boardId: number` | `DocumentDto`，目录项与正文分开给 |
| `save_board` | `boardId: number`、`document: DocumentDto` | `SaveResult`，整份写盘 |
| `create_board` | `title: string` | `BoardSummary`，空画板 |
| `rename_board` | `boardId: number`、`title: string` | `null` |
| `delete_board` | `boardId: number` | `null`，文件与缩略图进回收站，媒体保留 |

## 媒体与缩略图

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `read_media` | `hash: string` | 二进制响应，前端得到 ArrayBuffer |
| `write_media` | `name: string`、`data: number[]` | `string`，内容哈希 |
| `read_thumbnail` | `boardId: number` | 二进制响应；没有缩略图时是空字节 |
| `save_thumbnail` | `boardId: number`、`png: number[]` | `null`；PNG 由界面生成 |

## 册

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `list_volumes` | 无 | `VolumeSummary[]`，Empty book 固定排在最后 |
| `load_volume` | `volumeId: number` | `VolumeDto` |
| `save_volume` | `volumeId: number`、`volume: VolumeDto` | `null`，整份存；册里的页编号就是归属 |
| `create_volume` | `title: string` | `VolumeSummary`，空册 |
| `delete_volume` | `volumeId: number` | `null`，册里的页一起进回收站；Empty book 报 `cannotDeleteEmptyBook` |
| `assign_page` | `volumeId: number`、`page: number`、`at: number` | `number` 或 `null`，页原先所在的册（原来就在目标册时为 `null`）；`at` 超出范围时追加到末尾 |
| `unassign_page` | `page: number` | `number` 或 `null`，页原先所在的册 |
| `move_page` | `fromVolume: number`、`toVolume: number`、`page: number`、`at: number` | `null` |
| `move_page_in_volume` | `volumeId: number`、`from: number`、`to: number` | `null`，下标越界夹到边界 |
| `pages_in_volume` | `volumeId: number` | `number[]` |
| `volume_of_page` | `page: number` | `number` 或 `null`；页文件不在返回 `null`，在盘上没有归属算 Empty book（编号 0） |

## 书架

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `list_shelves` | 无 | `ShelfSummary[]` |
| `load_shelf` | `shelfId: number` | `ShelfDto` |
| `save_shelf` | `shelfId: number`、`shelf: ShelfDto` | `null`，只存册的引用 |
| `create_shelf` | `title: string` | `ShelfSummary`，空书架 |
| `delete_shelf` | `shelfId: number` | `null`，只删书架文件，册不动 |

## 回收站

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `list_trash` | 无 | `TrashItemDto[]`，按删除时间从新到旧，`daysLeft` 已算好 |
| `restore_trash` | `folder: string` | `null`；编号被占用报 `trashTargetExists` |
| `purge_trash_item` | `folder: string` | `null` |
| `empty_trash` | 无 | `number`，清掉的条数 |

## 导出与导入

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `export_container` | `boardId: number`、`fileName?: string` | `string`，导出文件的绝对路径 |
| `export_html` | `boardId: number`、`fileName?: string` | `string`，导出文件的绝对路径 |
| `import_container` | `path: string` | `number`，新画板的编号 |

## 错误

错误对象是 `{ kind, message }`。`kind` 是稳定字符串，前端按它分支；`message` 只用于显示。

| `kind` | 触发条件 |
| --- | --- |
| `workspaceNotOpen` | 工作区还没打开就调了其它命令 |
| `internal` | 命令层自身出错，例如状态锁被 poisoned |
| `io` | 文件读写失败 |
| `format` | 文件内容读不懂、缺字段或字段类型不对 |
| `outsideWorkspace` | 地址跑出工作区 |
| `badMediaName` | 媒体文件名不是内容哈希 |
| `badContainer` | 容器文件读不懂 |
| `newerFormat` | 文件格式号高于本程序支持的版本 |
| `cannotDeleteEmptyBook` | 删除 Empty book |
| `badTrash` | 回收站条目名不认识 |
| `trashTargetExists` | 还原时编号已经被占用 |
| `badFileName` | 导出文件名非法：空、`.`、`..`、含路径分隔符、含控制字符、以点或空格结尾、Windows 保留名 |

## 数据形状

- 字段一律 camelCase；块的正文是带 `kind` 标签的对象。
- `DocumentDto` 把元信息、目录项、正文分开列，正文按块编号与目录项对应；目录项可以没有正文，前端按空内容处理。

| 形状 | 字段 |
| --- | --- |
| `GeometryDto` | `x`、`y`、`width`、`height`、`rotation`（弧度，可省，按 0） |
| `EntryDto` | `id`、`kind`、`geometry`、`z`（可省，按 0）、`locked`（可省，按 false）、`visible`（可省，按 true） |
| `DocumentDto` | `board: { title, tags }`、`entries: EntryDto[]`、`bodies: BodyDto[]` |
| `BoardSummary` | `id`、`title`、`tags`、`updatedAt`（Unix 秒）、`entryCount` |
| `MediaSummary` | `hash`、`kind`、`name`、`bytes`、`boards`（用它画的画板数） |
| `SearchHitDto` | `boardId`、`title`、`snippet` |
| `SaveResult` | `updatedAt`（这次写盘用的 Unix 秒） |

应用与馆藏的形状：

| 形状 | 字段 |
| --- | --- |
| `WorkspaceStatus` | `root`（绝对路径）、`purgedOnOpen`（打开时清掉的过期回收站条目数） |
| `AppInfo` | `version`、`defaultWorkspace` |
| `VolumeDto` | `title`、`pages`（有序页编号） |
| `VolumeSummary` | `id`、`title`、`pageCount`、`updatedAt` |
| `ShelfDto` | `title`、`volumes`（有序册编号） |
| `ShelfSummary` | `id`、`title`、`volumeCount`、`updatedAt` |
| `TrashItemDto` | `folder`（还原与彻底删除都按它定位）、`kind`（board／volume／shelf）、`id`、`title`、`deletedAt`、`bytes`、`daysLeft`（0 表示已到期） |

`EntryDto.kind` 取 `text`、`image`、`video`、`audio`、`table`、`shape`、`highlight`。`BodyDto` 的 `kind` 取 `text`、`image`、`media`、`table`、`shape`、`highlight`；视频与音频共用 `media` 正文，种类由目录项决定。

| `BodyDto.kind` | 字段 |
| --- | --- |
| `text` | `id`、`text`、`font?`、`size?`、`color?`（`[r, g, b]`）、`background?` |
| `image` | `id`、`media`、`name`、`fit`（`cover`／`contain`／`fill`） |
| `media` | `id`、`media`、`name`、`bytes` |
| `table` | `id`、`rows`、`cols`、`cells`（行优先的文本数组） |
| `shape` | `id`、`shape`（`rect`／`ellipse`／`line`）、`fill?`、`stroke?`、`strokeWidth` |
| `highlight` | `id`、`color`、`opacity` |

```json
{
  "board": { "title": "示例画板", "tags": ["标签"] },
  "entries": [
    {
      "id": 1,
      "kind": "text",
      "geometry": { "x": 10.0, "y": 20.0, "width": 300.0, "height": 80.0, "rotation": 0.0 },
      "z": 0,
      "locked": false,
      "visible": true
    }
  ],
  "bodies": [
    {
      "kind": "text",
      "id": 1,
      "text": "正文",
      "font": null,
      "size": 16.0,
      "color": [44, 108, 200],
      "background": null
    }
  ]
}
```

## 媒体二进制

- `read_media` 与 `read_thumbnail` 走 Tauri 二进制响应：前端 `invoke` 得到 ArrayBuffer，不按 JSON 解析。
- `write_media` 与 `save_thumbnail` 用 JSON 数字数组收字节，每项 0 到 255。
- 媒体按内容哈希存取；同一份内容重复写入返回同一个哈希。
- 缩略图的空字节表示没有缩略图，前端按长度判断。
- 缩略图 PNG 由界面层生成，命令只负责落盘。

## 导出文件约定

- 导出写到 `<workspaceRoot>/exports/`，目录不存在就建，返回绝对路径。
- 省略 `fileName` 时默认名是 `<boardId>-<标题>.<扩展名>`，扩展名按命令取 `container` 或 `html`；标题里的 `/\:*?"<>|` 与控制字符换成 `_`，去掉首尾空白与结尾的点，清洗后为空用 `画板`。
- 显式 `fileName` 只接受纯文件名，原样使用、不补扩展名；非法时返回 `badFileName`。
- `import_container` 接受任意可读路径（用户从文件对话框选的容器），不要求在工作区内；导入生成新画板，编号由 `store` 分配，不覆盖原画板。
- `exports/` 不进索引，导出文件不参与列表、搜索与外部改动检测。

## 已知边界

- 没有自动保存：`save_board` 由界面决定何时调用，不调用就不落盘。
- 写媒体按 JSON 数字数组，适合图片；视频这类大文件待改成直接走原始请求体。
- `import_container` 可以读工作区外的文件，路径来自用户的文件对话框。
- `exports/` 不写索引，外部程序改导出目录不会被发现。
- 单元测试覆盖各模块里 `&Workspace` 层的普通函数；`tauri::State` 包装与命令注册没有测试，测试环境不启动 Tauri。
- 同时只支持一个工作区，打开新的会替换旧的。
