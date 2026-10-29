// 图标按模块单独取：`@lucide/vue` 的根入口会静态引入整份图标目录
// （3900 多个模块），开发期会把模块图撑到几千个请求，首屏要多等好几秒。
// 这里只列实际用到的图标，别名与包根保持一致。

import type { Component } from "vue";

import ArrowLeftIcon from "@lucide/vue/dist/esm/icons/arrow-left.mjs";
import BookOpenIcon from "@lucide/vue/dist/esm/icons/book-open.mjs";
import TrashIcon from "@lucide/vue/dist/esm/icons/trash.mjs";
import DownloadIcon from "@lucide/vue/dist/esm/icons/download.mjs";
import FileCodeIcon from "@lucide/vue/dist/esm/icons/file-code-corner.mjs";
import FilmIcon from "@lucide/vue/dist/esm/icons/film.mjs";
import HighlighterIcon from "@lucide/vue/dist/esm/icons/highlighter.mjs";
import ImagePlusIcon from "@lucide/vue/dist/esm/icons/image-plus.mjs";
import MaximizeIcon from "@lucide/vue/dist/esm/icons/maximize.mjs";
import MinusIcon from "@lucide/vue/dist/esm/icons/minus.mjs";
import MusicIcon from "@lucide/vue/dist/esm/icons/eighth-notes.mjs";
import PlusIcon from "@lucide/vue/dist/esm/icons/plus.mjs";
import RedoIcon from "@lucide/vue/dist/esm/icons/redo-2.mjs";
import SearchIcon from "@lucide/vue/dist/esm/icons/search.mjs";
import SquareIcon from "@lucide/vue/dist/esm/icons/square.mjs";
import GridIcon from "@lucide/vue/dist/esm/icons/layout-grid.mjs";
import TableIcon from "@lucide/vue/dist/esm/icons/table-2.mjs";
import TypeIcon from "@lucide/vue/dist/esm/icons/type.mjs";
import UndoIcon from "@lucide/vue/dist/esm/icons/undo-2.mjs";
import XIcon from "@lucide/vue/dist/esm/icons/x.mjs";

export const ArrowLeft = ArrowLeftIcon as Component;
export const BookOpen = BookOpenIcon as Component;
export const Trash = TrashIcon as Component;
export const Download = DownloadIcon as Component;
export const FileCode2 = FileCodeIcon as Component;
export const Film = FilmIcon as Component;
export const Highlighter = HighlighterIcon as Component;
export const ImagePlus = ImagePlusIcon as Component;
export const Maximize = MaximizeIcon as Component;
export const Minus = MinusIcon as Component;
export const Music = MusicIcon as Component;
export const Plus = PlusIcon as Component;
export const Redo2 = RedoIcon as Component;
export const Search = SearchIcon as Component;
export const Square = SquareIcon as Component;
export const Grid = GridIcon as Component;
export const Table2 = TableIcon as Component;
export const Type = TypeIcon as Component;
export const Undo2 = UndoIcon as Component;
export const X = XIcon as Component;