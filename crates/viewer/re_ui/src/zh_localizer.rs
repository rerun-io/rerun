use crate::localizer::Localizer;

pub struct ChineseLocalizer;

pub const CHINESE_LOCALIZER: ChineseLocalizer = ChineseLocalizer;

impl Localizer for ChineseLocalizer {
    fn t<'a>(&self, key: &'a str) -> &'a str {
        match key {
            // ── File operations ──
            "Open file…" => "打开文件…",
            "Open any supported files (.rrd, images, meshes, …) in a new recording" => {
                "打开任何支持的文件（.rrd、图片、网格等）到新记录中"
            }
            "Open from URL…" => "从 URL 打开…",
            "Open or navigate to data from any supported URL" => "从任何支持的 URL 打开或导航到数据",
            "Import into current recording…" => "导入到当前记录…",
            "Import any supported files (.rrd, images, meshes, …) in the current recording" => {
                "将任何支持的文件（.rrd、图片、网格等）导入到当前记录"
            }
            "Save recording…" => "保存记录…",
            "Save all data to a Rerun data file (.rrd)" => "将所有数据保存为 Rerun 数据文件（.rrd）",
            "Save current time selection…" => "保存当前时间选区…",
            "Save data for the current loop selection to a Rerun data file (.rrd)" => {
                "将当前循环选区的数据保存为 Rerun 数据文件（.rrd）"
            }
            "Save blueprint…" => "保存蓝图…",
            "Save the current viewer setup as a Rerun blueprint file (.rbl)" => {
                "将当前查看器设置保存为 Rerun 蓝图文件（.rbl）"
            }
            "Close current recording" => "关闭当前记录",
            "Close the current recording (unsaved data will be lost)" => "关闭当前记录（未保存的数据将丢失）",
            "Close all recordings" => "关闭所有记录",
            "Close all open current recording (unsaved data will be lost)" => {
                "关闭所有打开的记录（未保存的数据将丢失）"
            }
            "Next recording" => "下一个记录",
            "Switch to the next open recording" => "切换到下一个打开的记录",
            "Previous recording" => "上一个记录",
            "Switch to the previous open recording" => "切换到上一个打开的记录",
            "Back in history" => "后退",
            "Go back in history" => "在历史记录中后退",
            "Forward in history" => "前进",
            "Go forward in history" => "在历史记录中前进",
            "Undo" => "撤销",
            "Undo the last blueprint edit for the open recording" => "撤销对打开记录的最后一次蓝图编辑",
            "Redo" => "重做",
            "Redo the last undone thing" => "重做上一个撤销的操作",
            "Quit" => "退出",
            "Close the Rerun Viewer" => "关闭 Rerun 查看器",
            "rerun.io" => "rerun.io",
            "Visit our homepage" => "访问我们的主页",
            "Docs" => "文档",
            "Visit the docs on our website, with troubleshooting tips and more" => {
                "访问网站文档，包含故障排除技巧等"
            }
            "Rerun Discord" => "Rerun Discord",
            "Visit the Rerun Discord server, where you can ask questions and get help" => {
                "访问 Rerun Discord 服务器，在此提问和寻求帮助"
            }
            "Reset Viewer" => "重置查看器",
            "Reset the Viewer to how it looked the first time you ran it, forgetting UI state and all stored blueprints, except the ones loaded from *.rbl resources" => {
                "将查看器重置为首次运行时的状态，清除 UI 状态和所有存储的蓝图（.rbl 加载的除外）"
            }
            "Reset to default blueprint" => "重置为默认蓝图",
            "Clear active blueprint and use the default blueprint instead. If no default blueprint is set, this will use a heuristic blueprint." => {
                "清除当前蓝图并使用默认蓝图。如未设置默认蓝图，将使用启发式蓝图。"
            }
            "Reset to heuristic blueprint" => "重置为启发式蓝图",
            "Re-populate viewport with automatically chosen views using default visualizers" => {
                "使用默认可视化器自动选择视图重新填充视口"
            }
            "Open profiler" => "打开分析器",
            "Starts a profiler, showing what makes the viewer run slow" => "启动分析器，显示导致查看器运行缓慢的原因",
            "Capture profile trace…" => "捕获性能追踪…",
            "Capture profiling data and save them as a .puffin file" => "捕获性能分析数据并保存为 .puffin 文件",
            "Toggle memory panel" => "切换内存面板",
            "View and track current RAM usage inside Rerun Viewer" => "查看和跟踪 Rerun 查看器内的当前内存使用情况",
            "Toggle panel state overrides" => "切换面板状态覆盖",
            "Toggle panel state between app blueprint and overrides" => "在应用蓝图和覆盖之间切换面板状态",
            "Toggle top panel" => "切换顶栏",
            "Toggle the top panel" => "切换顶部面板",
            "Toggle blueprint panel" => "切换蓝图面板",
            "Toggle the left panel" => "切换左侧面板",
            "Expand blueprint panel" => "展开蓝图面板",
            "Expand the left panel" => "展开左侧面板",
            "Toggle selection panel" => "切换选择面板",
            "Toggle the right panel" => "切换右侧面板",
            "Expand selection panel" => "展开选择面板",
            "Expand the right panel" => "展开右侧面板",
            "Toggle time panel" => "切换时间面板",
            "Toggle the bottom panel" => "切换底部面板",
            "Toggle chunk store browser" => "切换数据块存储浏览器",
            "Toggle the chunk store browser" => "切换数据块存储浏览器",
            "Settings…" => "设置…",
            "Show the settings screen" => "显示设置界面",
            "Toggle blueprint inspection panel" => "切换蓝图检查面板",
            "Inspect the timeline of the internal blueprint data." => "检查内部蓝图数据的时间线。",
            "Toggle egui debug panel" => "切换 egui 调试面板",
            "View and change global egui style settings" => "查看和更改全局 egui 样式设置",
            "Toggle fullscreen" => "切换全屏",
            "Toggle between windowed and fullscreen viewer" => "在窗口和全屏查看器之间切换",
            "Toggle between full viewport dimensions and initial dimensions" => "在完整视口尺寸和初始尺寸之间切换",
            "Zoom in" => "放大",
            "Increases the UI zoom level" => "增加 UI 缩放级别",
            "Zoom out" => "缩小",
            "Decreases the UI zoom level" => "减小 UI 缩放级别",
            "Reset zoom" => "重置缩放",
            "Resets the UI zoom level to the operating system's default value" => {
                "将 UI 缩放级别重置为操作系统默认值"
            }
            "Command palette…" => "命令面板…",
            "Toggle the Command Palette" => "切换命令面板",
            "Toggle play/pause" => "播放/暂停",
            "Either play or pause the time" => "播放或暂停时间轴",
            "Follow" => "跟随",
            "Follow on from end of timeline" => "从时间线末尾继续跟随",
            "Step backwards" => "上一步",
            "Move the time marker back to the previous point in time with any data" => {
                "将时间标记移回有数据的上一个时间点"
            }
            "Step forwards" => "下一步",
            "Move the time marker to the next point in time with any data" => {
                "将时间标记移到有数据的下一个时间点"
            }
            "Move backwards" => "向后移动",
            "Move the time marker backward by 1 second" => "将时间标记向后移动 1 秒",
            "Move forwards" => "向前移动",
            "Move the time marker forward by 0.1 seconds" => "将时间标记向前移动 0.1 秒",
            "Move backwards fast" => "快速后退",
            "Move the time marker backwards by 1 second" => "将时间标记向后移动 1 秒",
            "Move forwards fast" => "快速前进",
            "Move the time marker forwards by 0.1 seconds" => "将时间标记向前移动 0.1 秒",
            "Go to beginning" => "跳到开头",
            "Go to beginning of timeline" => "跳到时间线开始",
            "Go to end" => "跳到末尾",
            "Go to end of timeline" => "跳到时间线末尾",
            "Restart" => "重新开始",
            "Restart from beginning of timeline" => "从时间线开头重新开始",
            "Set playback speed" => "设置播放速度",
            "This is a chord, so you can press 5+0 to set the speed to 50x" => {
                "这是和弦键，可依次按 5+0 将速度设为 50 倍"
            }
            "Screenshot" => "截图",
            "Copy screenshot of the whole app to clipboard" => "将整个应用的截图复制到剪贴板",
            "Print datastore" => "打印数据存储",
            "Prints the entire chunk store to the console and clipboard. WARNING: this may be A LOT of text." => {
                "将整个数据块存储打印到控制台和剪贴板。警告：这可能产生大量文本。"
            }
            "Print blueprint store" => "打印蓝图存储",
            "Prints the entire blueprint store to the console and clipboard. WARNING: this may be A LOT of text." => {
                "将整个蓝图存储打印到控制台和剪贴板。警告：这可能产生大量文本。"
            }
            "Print primary cache" => "打印主缓存",
            "Prints the state of the entire primary cache to the console and clipboard. WARNING: this may be A LOT of text." => {
                "将整个主缓存的状态打印到控制台和剪贴板。警告：这可能产生大量文本。"
            }
            "Reset egui memory" => "重置 egui 内存",
            "Reset egui memory, useful for debugging UI code." => "重置 egui 内存，用于调试 UI 代码。",
            "Share…" => "分享…",
            "Share the current screen as a link" => "将当前画面分享为链接",
            "Copy direct link" => "复制直接链接",
            "Try to copy a shareable link to the current screen. This is not supported for all data sources & viewer states." => {
                "尝试复制当前画面的可分享链接。并非所有数据源和查看器状态都支持此功能。"
            }
            "Copy link to selected time range" => "复制所选时间范围的链接",
            "Copy a link to the part of the active recording within the loop selection bounds." => {
                "复制活动记录中循环选区范围内部分的链接。"
            }
            "Copy entity hierarchy" => "复制实体层级",
            "Copy the complete entity hierarchy tree of the currently active recording to the clipboard." => {
                "将当前活动记录的完整实体层级树复制到剪贴板。"
            }
            "Restart with WebGL" => "使用 WebGL 重新启动",
            "Reloads the webpage and force WebGL for rendering. All data will be lost." => {
                "重新加载网页并强制使用 WebGL 渲染。所有数据将丢失。"
            }
            "Restart with WebGPU" => "使用 WebGPU 重新启动",
            "Reloads the webpage and force WebGPU for rendering. All data will be lost." => {
                "重新加载网页并强制使用 WebGPU 渲染。所有数据将丢失。"
            }
            "Connect to a server…" => "连接到服务器…",
            "Connect to a Redap server (experimental)" => "连接到 Redap 服务器（实验性）",

            // ── Welcome screen ──
            "The data layer for physical AI" => "物理 AI 的数据层",
            "Log multi-rate, multimodal data with the Rerun SDK in C++, Python, or Rust" => {
                "使用 C++、Python 或 Rust 的 Rerun SDK 记录多速率、多模态数据"
            }
            "Visualize and explore live or recorded data across the pipeline" => {
                "可视化和探索流水线中的实时或记录数据"
            }
            "Query with dataframes or SQL, and stream directly to training" => {
                "使用数据框或 SQL 查询，并直接流式传输到训练"
            }
            "Go to documentation →" => "查看文档 →",
            "Connecting to data source" => "正在连接到数据源",
            "Loading" => "正在加载",
            "Send data in" => "数据输入",
            "Ingest multi-rate, multimodal data from robot logs, sensors, simulation, or video." => {
                "从机器人日志、传感器、仿真或视频中摄取多速率、多模态数据。"
            }
            "Explore data" => "数据探索",
            "Visualize and explore multi-rate, multimodal data across every stage of the pipeline." => {
                "可视化和探索流水线各阶段的多速率、多模态数据。"
            }
            "Query data out" => "数据查询",
            "Query raw, intermediate, and derived data with dataframes or SQL, and stream to training." => {
                "使用数据框或 SQL 查询原始、中间和衍生数据，并流式传输到训练。"
            }
            "Rerun Hub" => "Rerun Hub",
            "The production backend for the Rerun data layer — turn your object stores into a queryable, streamable foundation. " => {
                "Rerun 数据层的生产后端——将您的对象存储转变为可查询、可流式传输的基础。 "
            }
            "Learn more" => "了解更多",
            " or " => " 或 ",
            "book a demo" => "预约演示",
            "." => "。",
            "Add server and login" => "添加服务器并登录",
            "Add server" => "添加服务器",
            "logged in as " => "已登录为 ",
            "Add credentials" => "添加凭据",
            "for address " => "地址 ",
            "Explore your data" => "探索您的数据",
            "Log out" => "退出登录",
            "Hi," => "你好，",
            "Fetching example list" => "正在获取示例列表",
            "No examples found." => "未找到示例。",
            "View example recordings" => "查看示例记录",
            "Source code" => "源代码",
            "Source code is not available for this example" => "此示例的源代码不可用",

            // ── Settings screen ──
            "Settings" => "设置",
            "Close" => "关闭",
            "General" => "常规",
            "Theme" => "主题",
            "Memory budget" => "内存预算",
            "When this limit is reached we start purging data from RAM" => "达到此限制时，将开始从 RAM 中清除数据",
            "Prefetch" => "预取",
            "Controls how aggressively we prefetch chunks ahead of what is strictly needed.\n\n• Required: only chunks required to render the current time cursor.\n• Similar: also prefetch chunks on the same component paths as required chunks up to a given real-time duration.\n• Everything: also prefetch every chunk in the recording." => {
                "控制预取块的激进程度。\n\n• Required：仅预取渲染当前时间光标所需的块。\n• Similar：还预取与所需块相同组件路径上的块，最多到给定的实时时长。\n• Everything：预取记录中的每个块。"
            }
            "Show 'Rerun examples' button" => "显示「Rerun 示例」按钮",
            "Limit number of primitives in a view" => "限制视图中的基本体数量",
            "Caps the number of elements individual visualizers process (e.g. instance caps for 3D shapes, line limits for time series). Disabling this may cause the viewer to become unresponsive with very large data sets." => {
                "限制单个可视化器处理的元素数量（例如 3D 形状的实例上限、时间序列的线条限制）。禁用此选项可能导致查看器在数据集过大时无响应。"
            }
            "Timestamp format" => "时间戳格式",
            "Title bar" => "标题栏",
            "Use custom window decorations" => "使用自定义窗口装饰",
            "Hide the native title bar and draw Rerun's top bar as the window frame.\n\n Opt out of this if you experience any issues with the window's behavior." => {
                "隐藏系统标题栏，将 Rerun 顶栏绘制为窗口边框。\n\n如果在窗口行为方面遇到任何问题，请关闭此选项。"
            }
            "Show performance metrics" => "显示性能指标",
            "Show metrics for milliseconds/frame and RAM usage in the top bar" => "在顶栏中显示毫秒/帧和内存使用指标",
            "Show notification toasts" => "显示通知提示",
            "Show toasts for log messages and other notifications" => "显示日志消息和其他通知的提示",
            "Map view" => "地图视图",
            "Video" => "视频",
            "Experimental" => "实验性",
            "Table cards and blueprints" => "表格卡片和蓝图",
            "Enable table blueprints embedded in Arrow schema metadata, plus grid view mode for server supplied tables.\n\n When enabled, tables can carry inline view definitions for segment previews, and a list/grid toggle appears in the table title bar." => {
                "启用嵌入在 Arrow schema 元数据中的表格蓝图，以及服务器提供表格的网格视图模式。\n\n启用后，表格可携带用于片段预览的内联视图定义，并在表格标题栏显示列表/网格切换。"
            }
            "Required" => "必需",
            "Similar" => "相似",
            "Everything" => "全部",
            "UTC" => "UTC",
            "Local (show time zone)" => "本地（显示时区）",
            "Local (hide time zone)" => "本地（隐藏时区）",
            "Note: timestamps without time zone are ambiguous when copied elsewhere." => "注意：不包含时区的时间戳在复制到其他地方时会存在歧义。",
            "Seconds since Unix epoch" => "Unix 纪元以来的秒数",
            "Mapbox access token:" => "Mapbox 访问令牌：",
            "This token is used to enable Mapbox-based map view backgrounds.\n\nNote that the token will be saved in clear text in the configuration file. The token can also be set using the RERUN_MAPBOX_ACCESS_TOKEN environment variable." => {
                "此令牌用于启用基于 Mapbox 的地图视图背景。\n\n注意：令牌将以明文形式保存在配置文件中。也可以使用 RERUN_MAPBOX_ACCESS_TOKEN 环境变量设置令牌。"
            }
            "Override the FFmpeg binary path" => "覆盖 FFmpeg 二进制路径",
            "By default, the viewer tries to automatically find a suitable FFmpeg binary in the system's `PATH`. Enabling this option allows you to specify a custom path to the FFmpeg binary." => {
                "默认情况下，查看器会尝试在系统的 `PATH` 中自动查找合适的 FFmpeg 二进制文件。启用此选项可指定自定义路径。"
            }
            "Path:" => "路径：",
            "Decoder:" => "解码器：",
            "Checking FFmpeg version" => "正在检查 FFmpeg 版本",
            "FFmpeg found" => "找到 FFmpeg",
            "Incompatible FFmpeg version" => "不兼容的 FFmpeg 版本",
            "FFmpeg binary found but unable to parse version" => "找到 FFmpeg 二进制文件但无法解析版本",
            "The specified FFmpeg binary path does not exist or is not a file." => "指定的 FFmpeg 二进制路径不存在或不是文件。",
            "unlimited" => "无限制",

            // ── Selection panel / Time panel ──
            " + Scroll" => " + 滚动",
            " FPS" => " 帧/秒",
            " in view " => " 在视图中 ",
            " offset" => " 偏移",
            " with " => " 与 ",
            "# Component defaults\nThis section lists default values for components in the scope of the present view. The visualizers corresponding to this view's entities use these defaults when no per-entity store value or override is specified.\nClick on the `+` button to add a new default value." => "# 组件默认值\n此部分列出当前视图范围内的组件默认值。当没有指定按实体存储的值或覆盖时，对应视图实体的可视化器将使用这些默认值。\n点击 `+` 按钮添加新的默认值。",
            "# Visualizers\n\nThis section lists all active visualizers in this view." => "# 可视化器\n\n此部分列出此视图中所有活动的可视化器。",
            "(Not shown in any view)" => "（未在任何视图中显示）",
            "(default)" => "（默认）",
            "(none)" => "（无）",
            "(unknown visualizer type)" => "（未知的可视化器类型）",
            "/s" => "/秒",
            "100 fits in NonMinI64" => "100 适合 NonMinI64",
            "Add a new view or container to this container" => "向此容器添加新视图或容器",
            "Add a new visualizer to the current view." => "向当前视图添加新可视化器。",
            "Add additional visualizers" => "添加更多可视化器",
            "Add custom" => "添加自定义",
            "Add descendants of this entity to the view" => "将此实体的后代添加到视图",
            "Add more component defaults" => "添加更多组件默认值",
            "Add new visualizer…" => "添加新可视化器…",
            "Add overrides…" => "添加覆盖…",
            "Add/remove Entities" => "添加/移除实体",
            "All components already have active defaults" => "所有组件已有活动默认值",
            "Application ID:" => "应用 ID：",
            "Archetype" => "原型",
            "At" => "在",
            "Auto" => "自动",
            "Blueprint Streams" => "蓝图流",
            "Can't share links to the current recording" => "无法分享当前记录的链接",
            "Can't share links to the current recording:" => "无法分享当前记录的链接：",
            "Clear blueprint component" => "清除蓝图组件",
            "Clone this view" => "克隆此视图",
            "Columns" => "列",
            "Component" => "组件",
            "Component defaults" => "组件默认值",
            "Component type" => "组件类型",
            "Connection throughput" => "连接吞吐量",
            "Container kind" => "容器类型",
            "Contents" => "内容",
            "Coordinate frame" => "坐标框架",
            "Copied timeline:" => "已复制时间线：",
            "Copy link to time selection" => "复制时间选区链接",
            "Copy link to timestamp" => "复制时间戳链接",
            "Copy timeline name" => "复制时间线名称",
            "Copy timestamp" => "复制时间戳",
            "Create an exact duplicate of this view including all blueprint settings" => "创建此视图的精确副本，包含所有蓝图设置",
            "Current override is the same as the override specified in the default blueprint (if any)" => "当前覆盖与默认蓝图指定的覆盖相同（如果有）",
            "Custom" => "自定义",
            "Data" => "数据",
            "Default" => "默认",
            "Default query range settings for this kind of view" => "此类视图的默认查询范围设置",
            "Distribute content equally" => "平均分配内容",
            "Does not match any entity" => "不匹配任何实体",
            "Does not perform a latest-at query, shows only data logged at exactly the current time cursor position." => "不执行最新时间查询，仅显示在精确的当前时间光标位置记录的数据。",
            "Double click" => "双击",
            "Downloading meta-data" => "正在下载元数据",
            "Drag time scale" => "拖动时间刻度",
            "Entire timeline" => "整个时间线",
            "Entity not found in view" => "视图中未找到实体",
            "Entity path" => "实体路径",
            "Entity path filter" => "实体路径筛选器",
            "Exclude entity" => "排除实体",
            "Exclude this entity and all its descendants from the view" => "从视图中排除此实体及其所有后代",
            "Failed to find container in blueprint" => "在蓝图中未找到容器",
            "Failed to find view in blueprint" => "在蓝图中未找到视图",
            "Frames per second" => "帧每秒",
            "From" => "从",
            "Full documentation" => "完整文档",
            "Go to default timeline" => "跳到默认时间线",
            "If disabled, the entity will not react to any mouse interaction." => "禁用后，实体将不对任何鼠标交互做出响应。",
            "If disabled, the entity won't be shown in the view." => "禁用后，实体将不会在视图中显示。",
            "Include entity" => "包含实体",
            "Index type" => "索引类型",
            "Instance" => "实例",
            "Interactive" => "可交互",
            "Latest-at query at: " => "最新时间查询于：",
            "Length" => "长度",
            "Looping entire recording" => "循环播放整个记录",
            "Looping is off" => "循环播放关闭",
            "Looping selection" => "循环播放选区",
            "Make all children the same size" => "使所有子元素等大",
            "Make default for current view" => "设为当前视图默认",
            "Matches" => "匹配",
            "Matches 1 entity" => "匹配 1 个实体",
            "Middle click drag" => "中键拖动",
            "Modify the entity query using the editor" => "使用编辑器修改实体查询",
            "More options" => "更多选项",
            "Name" => "名称",
            "No active timeline" => "无活动时间线",
            "No additional visualizers available" => "没有可用的其他可视化器",
            "No components to visualize" => "没有要可视化的组件",
            "No entities match the filter." => "没有实体匹配筛选器。",
            "No event logged on timeline" => "时间线上没有记录事件",
            "No properties found for this recording." => "此记录未找到属性。",
            "One other timeline has data" => "还有 1 条时间线有数据",
            "Open the context menu on selected time to copy link" => "在选中的时间上打开上下文菜单以复制链接",
            "Open the context menu on selected time to remove it" => "在选中的时间上打开上下文菜单以移除",
            "Open the context menu on selected time to save it" => "在选中的时间上打开上下文菜单以保存",
            "Other values:" => "其他值：",
            "Other:" => "其他：",
            "Override" => "覆盖",
            "Pan" => "平移",
            "Pan horizontally" => "水平平移",
            "Pan vertically" => "垂直平移",
            "Parent entity" => "父实体",
            "Play/Pause" => "播放/暂停",
            "Playback speed" => "播放速度",
            "Properties" => "属性",
            "Query range settings inherited from enclosing view" => "从外部视图继承的查询范围设置",
            "Recommended:" => "推荐：",
            "Recording ID:" => "记录 ID：",
            "Remove this container" => "移除此容器",
            "Remove this rule" => "移除此规则",
            "Remove this view" => "移除此视图",
            "Remove time selection" => "移除时间选区",
            "Remove visualizer" => "移除可视化器",
            "Rerun lacks edit UI for:" => "Rerun 缺少编辑 UI：",
            "Reset override to default blueprint" => "将覆盖重置为默认蓝图",
            "Reset view" => "重置视图",
            "Resets the override to what is specified in the default blueprint" => "将覆盖重置为默认蓝图中的设置",
            "Right click drag" => "右键拖动",
            "Scroll" => "滚动",
            "Search for entity…" => "搜索实体…",
            "Select time segment" => "选择时间段",
            "Selection" => "选择",
            "Set query range settings for the contents of this view" => "为此视图的内容设置查询范围",
            "Set query range settings for this entity" => "为此实体设置查询范围",
            "Shown in" => "显示于",
            "Simplify hierarchy" => "简化层级",
            "Simplify this container and its children" => "简化此容器及其子元素",
            "Snap to grid" => "对齐网格",
            "Source" => "来源",
            "Space origin" => "空间原点",
            "Start" => "开始",
            "State got created just now" => "状态刚刚创建",
            "Static" => "静态",
            "Stop" => "停止",
            "Stop excluding this entity path." => "停止排除此实体路径。",
            "Stop including this entity path." => "停止包含此实体路径。",
            "Store kind:" => "存储类型：",
            "Stream entity" => "流实体",
            "Streams" => "流",
            "Temporal" => "时序",
            "The current recording doesn't support time selection links" => "当前记录不支持时间选区链接",
            "The current recording doesn't support time stamp links" => "当前记录不支持时间戳链接",
            "The full timeline of the recording, which may be bigger than the data range of this plot" => "记录的完整时间线，可能大于此图表的数据范围",
            "The recording has no timeline" => "记录没有时间线",
            "The recording has no timelines" => "记录没有时间线",
            "The type of this view" => "此视图的类型",
            "There are no visualizers available to add to this view." => "没有可添加到此视图的可视化器。",
            "This view is experimental: its API, behavior, and on-disk format may change without notice." => "此视图为实验性：其 API、行为和磁盘格式可能随时更改，恕不另行通知。",
            "This view is not able to visualize any of the matched entities using the current root" => "此视图无法使用当前根节点可视化任何匹配的实体",
            "Timeline" => "时间线",
            "Timestamp" => "时间戳",
            "To" => "到",
            "Transform relation can't be resolved due to empty coordinate frame name." => "由于坐标框架名称为空，无法解析变换关系。",
            "Unknown container" => "未知容器",
            "Unknown view" => "未知视图",
            "Uses the latest known value for each component." => "使用每个组件的最新已知值。",
            "View default" => "视图默认值",
            "View properties" => "视图属性",
            "View type" => "视图类型",
            "Visible" => "可见",
            "Visible time range" => "可见时间范围",
            "Visualizers" => "可视化器",
            "Waiting for timeline" => "等待时间线",
            "Without archetype" => "无原型",
            "Zoom" => "缩放",
            "absolute time" => "绝对时间",
            "beginning of timeline" => "时间线开始",
            "component" => "组件",
            "component, logged" => "组件，已记录",
            "container" => "容器",
            "container_grid_columns" => "container_grid_columns",
            "current frame" => "当前帧",
            "current time" => "当前时间",
            "empty — use the + button to add content" => "空 — 使用 + 按钮添加内容",
            "end of timeline" => "时间线末尾",
            "entities" => "实体",
            "frame" => "帧",
            "hidden suggestions" => "个隐藏建议",
            "non-empty" => "非空",
            "none" => "无",
            "of entity" => "的实体",
            "once" => "一次",
            "other timelines have data" => "条其他时间线有数据",
            "rows" => "行",
            "selected items" => "个选中项",
            "selection_panel_component_hybrid_overwrite" => "selection_panel_component_hybrid_overwrite",
            "selection_panel_component_static" => "selection_panel_component_static",
            "selection_panel_component_static_overwrite" => "selection_panel_component_static_overwrite",
            "selection_panel_component_temporal_latest_all" => "selection_panel_component_temporal_latest_all",
            "selection_panel_component_temporal_latest_all_multi_instance" => "selection_panel_component_temporal_latest_all_multi_instance",
            "selection_panel_entity_static_latest_all" => "selection_panel_entity_static_latest_all",
            "selection_panel_entity_temporal_latest_all" => "selection_panel_entity_temporal_latest_all",
            "selection_panel_recording" => "selection_panel_recording",
            "selection_panel_recording_hover_app_id" => "selection_panel_recording_hover_app_id",
            "selection_panel_view" => "selection_panel_view",
            "selection_panel_view_entity_no_match" => "selection_panel_view_entity_no_match",
            "selection_panel_view_entity_no_visualizable" => "selection_panel_view_entity_no_visualizable",
            "selection_view" => "selection_view",
            "timeline" => "时间线",
            "times" => "次",
            "values" => "个值",
            "view" => "视图",
            "x" => "x",

            // ── Top panel ──
            "Minimize" => "最小化",
            "Maximize" => "最大化",
            "Selection panel toggle" => "切换选择面板",
            "Time panel toggle" => "切换时间面板",
            "Blueprint panel toggle" => "切换蓝图面板",
            "⚠ Debug build" => "⚠ 调试构建",
            "Rerun was compiled with debug assertions enabled." => "Rerun 在启用调试断言的情况下编译。",
            "⚠ Docker" => "⚠ Docker",
            "It looks like the Rerun Viewer is running inside a Docker container. This is not officially supported, and may lead to subtle bugs. " => {
                "Rerun 查看器似乎正在 Docker 容器内运行。此情况未正式支持，可能导致微妙的错误。"
            }
            "Click for more info." => "点击了解更多信息。",
            "⚠ Software rasterizer" => "⚠ 软件光栅化",
            "Software rasterizer detected - expect poor performance." => "检测到软件光栅化——性能可能较差。",
            "Rerun requires hardware accelerated graphics (i.e. a GPU) for good performance." => {
                "Rerun 需要硬件加速图形（即 GPU）才能获得良好性能。"
            }
            "Click for troubleshooting." => "点击查看故障排除。",
            "wgpu adapter" => "wgpu 适配器",
            "A blinking orange dot appears here in debug builds whenever request_discard is called.\n\
        It is expect that the dot appears occasionally, e.g. when showing a new panel for the first time.\n\
        However, it should not be sustained, as that would indicate a performance bug." => {
                "调试构建中，每当调用 request_discard 时，此处会出现闪烁的橙色圆点。\n偶尔出现是正常的，例如首次显示新面板时。\n但不应该持续出现，否则表示存在性能问题。"
            }
            "no connection to" => "未连接到",
            "latency for" => "延迟",
            "CPU time used by Rerun Viewer each frame. Lower is better." => "Rerun 查看器每帧使用的 CPU 时间。越低越好。",
            "Frames per second. Higher is better." => "每秒帧数。越高越好。",
            "See memory panel for more info" => "查看内存面板以获取更多信息",
            "Viewer" => "查看器",
            "External" => "外部",
            "Allocations" => "分配数",
            "GPU" => "GPU",
            "GPU textures" => "GPU 纹理",
            "GPU buffers" => "GPU 缓冲区",
            "Rerun Viewer is using {} of Resident memory (RSS),\nplus {} of GPU memory in {} textures and {} buffers." => {
                "Rerun 查看器正在使用 {} 常驻内存（RSS），\n以及 {} GPU 内存（{} 个纹理和 {} 个缓冲区）。"
            }
            "The Rerun viewer was not configured to run with an AccountingAllocator,\nconsider adding the following to your code's main entrypoint:" => {
                "Rerun 查看器未配置为使用 AccountingAllocator 运行，\n请考虑在代码的入口点添加以下内容："
            }
            "To get more accurate memory reportings, consider configuring your Rerun \nviewer to use an AccountingAllocator by adding the following to your \ncode's main entrypoint:" => {
                "为获取更准确的内存报告，请考虑在代码的入口点添加以下配置，\n使 Rerun 查看器使用 AccountingAllocator："
            }
            "(click to copy to clipboard)" => "（点击复制到剪贴板）",
            "No latency data available." => "无可用延迟数据。",
            "End-to-end latency from when the data was logged by the SDK to when it is shown in the viewer.\nThis includes time for encoding, network latency, and decoding.\nIt is also affected by the framerate of the viewer.\nThis latency is inaccurate if the logging was done on a different machine, since it is clock-based." => {
                "端到端延迟，从 SDK 记录数据到查看器显示数据的时间。\n包括编码、网络延迟和解码时间。\n还受查看器帧率的影响。\n如果记录在不同机器上完成，由于时钟差异，此延迟可能不准确。"
            }
            "end-to-end:" => "端到端：",
            "N/A MiB" => "N/A MiB",
            "Latency:" => "延迟：",
            // ── Memory panel / Datastore browser ──
            "# Bind Group Layouts:" => "# 绑定组布局：",
            "# Bind Groups:" => "# 绑定组：",
            "# Buffers:" => "# 缓冲区：",
            "# Pipeline Layouts:" => "# 管线布局：",
            "# Render Pipelines:" => "# 渲染管线：",
            "# rows" => "# 行",
            "# Samplers:" => "# 采样器：",
            "# Shader Modules:" => "# 着色器模块：",
            "# Textures:" => "# 纹理：",
            "Access token" => "访问令牌",
            "Access token (will be stored in plain text):" => "访问令牌（将以明文存储）：",
            "Account login" => "账户登录",
            "Account login:" => "账户登录：",
            "Actual size" => "实际大小",
            "Add" => "添加",
            "Address:" => "地址：",
            "All" => "全部",
            "All required loaded" => "所有必需已加载",
            "all smaller than" => "均小于",
            "Allocation tracking" => "分配追踪",
            "Allocator" => "分配器",
            "allocs" => "分配数",
            "Application ID" => "应用 ID",
            "At:" => "在：",
            "Authentication:" => "身份验证：",
            "Bandwidth" => "带宽",
            "Bandwidth (B/s)" => "带宽（字节/秒）",
            "Batches" => "批次数",
            "Batches canceled in the last second (e.g. due to time cursor movement)" => "上一秒取消的批次数（如因时间光标移动）",
            "BGR:" => "BGR：",
            "BGRA:" => "BGRA：",
            "Blueprint" => "蓝图",
            "Blueprints" => "蓝图",
            "Buffer Memory:" => "缓冲区内存：",
            "Cancel" => "取消",
            "Cancellations" => "已取消",
            "Chunk count" => "数据块计数",
            "Chunk ID" => "数据块 ID",
            "Chunk max byte" => "数据块最大字节",
            "Chunk max rows" => "数据块最大行数",
            "Chunk max rows (unsorted)" => "数据块最大行数（未排序）",
            "Chunk replaced by a compacted version" => "数据块被压缩版本替换",
            "chunk_store_info_scroll_area" => "chunk_store_info_scroll_area",
            "Chunks" => "数据块",
            "Chunks GC'd" => "已 GC 的数据块",
            "Chunks in flight" => "传输中的数据块",
            "Chunks removed" => "已移除的数据块",
            "Clear filters" => "清除筛选器",
            "Click to close and copy contents" => "点击关闭并复制内容",
            "Click to copy callstack to clipboard" => "点击复制调用栈到剪贴板",
            "Close datastore browser" => "关闭数据存储浏览器",
            "Close the datastore browser" => "关闭数据存储浏览器",
            "Compaction" => "压缩",
            "Component:" => "组件：",
            "Components" => "组件",
            "Config" => "配置",
            "Connection" => "连接",
            "Connection state to the redap server" => "与 redap 服务器的连接状态",
            "Continue as" => "继续为",
            "Copy chunks" => "复制数据块",
            "Copy the currently listed chunks as text" => "将当前列出的数据块复制为文本",
            "counted = fully_tracked + stochastically_tracked + untracked + overhead" => {
                "已计数 = 完全追踪 + 随机追踪 + 未追踪 + 开销"
            },
            "Counts" => "计数",
            "Datastore Resources" => "数据存储资源",
            "Depth:" => "深度：",
            "Details" => "详情",
            "Dismiss" => "忽略",
            "Dismiss all" => "全部忽略",
            "Display all chunks" => "显示所有数据块",
            "Display chunks relevant to the provided latest-at query" => "显示与提供的 latest-at 查询相关的数据块",
            "Display chunks relevant to the provided range query" => "显示与提供的 range 查询相关的数据块",
            "Display the current recording's blueprint store" => "显示当前记录的蓝图存储",
            "Display the current recording's data store" => "显示当前记录的数据存储",
            "Don't show again" => "不再显示",
            "Double-click to reset view, scroll to zoom, drag to pan." => "双击重置视图，滚动缩放，拖动平移。",
            "Downloading chunks" => "正在下载数据块",
            "Edit server:" => "编辑服务器：",
            "Effective size" => "有效大小",
            "Enable changelog" => "启用变更日志",
            "Enable detailed allocation tracking" => "启用详细分配追踪",
            "Entities" => "实体",
            "Entity" => "实体",
            "Entity:" => "实体：",
            "Events" => "事件",
            "Events (total)" => "事件（总计）",
            "Flamegraph" => "火焰图",
            "Flip" => "翻转",
            "From:" => "从：",
            "fully_tracked:" => "完全追踪：",
            "Garbage collection" => "垃圾回收",
            "Generation" => "代",
            "GPU Memory" => "GPU 内存",
            "GPU Resources" => "GPU 资源",
            "Heap size" => "堆大小",
            "height" => "高度",
            "Hide dimension slider" => "隐藏维度滑块",
            "Host name" => "主机名",
            "How many chunks in the cache?" => "缓存中有多少个数据块？",
            "http" => "http",
            "https" => "https",
            "Image" => "图像",
            "in" => "内",
            "In-flight entities" => "传输中的实体",
            "In-memory size of the manifest (the index of all chunks)" => "清单（所有数据块的索引）的内存大小",
            "Include Resident Set Size (RSS) in the flamegraph. This shows total memory use as reported by the OS. This may be a lot bigger than what is actually _used_ because our allocator (mimalloc) retains pages in case they are needed again." => {
                "在火焰图中包含常驻集大小（RSS）。这显示操作系统报告的总内存使用。这可能远大于实际使用的内存，因为分配器（mimalloc）会保留页面以备再次使用。"
            },
            "Include RSS" => "包含 RSS",
            "Index size (in memory)" => "索引大小（内存中）",
            "Info" => "信息",
            "L:" => "L：",
            "Label:" => "标签：",
            "Latest-at" => "最新时间",
            "latest_at" => "latest_at",
            "LatestAt" => "最新时间查询",
            "Limit" => "限制",
            "Loaded" => "已加载",
            "Loaded protected roots" => "已加载的保护根",
            "log out" => "退出登录",
            "Manifest" => "清单",
            "Manifest advertised" => "已通告的清单",
            "Manifest size (in memory)" => "清单大小（内存中）",
            "Memory budget filled" => "内存预算已满",
            "Memory flamegraph visualizing the memory usage tree." => "内存火焰图，可视化内存使用树。",
            "Memory pressure eviction" => "内存压力驱逐",
            "No active server streaming connections." => "没有活动的服务器流连接。",
            "No authentication" => "无身份验证",
            "No callstack available" => "无可用调用栈",
            "No fetch data yet" => "尚未获取数据",
            "No memory usage tree available." => "无可用内存使用树。",
            "No notifications yet." => "暂无通知。",
            "No store statistics available." => "无可用存储统计信息。",
            "No value" => "无值",
            "Notification toggle" => "通知切换",
            "Notifications" => "通知",
            "now" => "现在",
            "Number of in-flight request batches to the server" => "发送到服务器的传输中请求批次数",
            "Number of non-null component batches (cells)" => "非空组件批次数（单元格）",
            "Old split chunks removed when their root chunk was re-downloaded" => "根数据块重新下载时移除的旧拆分数据块",
            "or" => "或",
            "Over time" => "随时间",
            "Overall" => "总体",
            "overhead:" => "开销：",
            "Overwrite" => "覆盖",
            "Pending bytes" => "待处理字节",
            "Pending requests" => "待处理请求",
            "Position:" => "位置：",
            "Preview" => "预览",
            "Primary Query Caches" => "主查询缓存",
            "Prioritization" => "优先级",
            "Progress" => "进度",
            "Protected physical" => "受保护的物理内存",
            "Protected root chunks are root chunks that won't be GC'd. A chunk is typically protected if it's actively in use." => {
                "受保护的根数据块是不会被垃圾回收的根数据块。数据块通常在使用中时受保护。"
            },
            "Protected roots" => "受保护的根",
            "Query caches" => "查询缓存",
            "RAM purge" => "RAM 清除",
            "Range" => "范围",
            "range" => "范围",
            "Recent average download speed (compressed on-wire bytes)" => "近期平均下载速度（压缩传输字节）",
            "Recently canceled" => "最近取消的",
            "Recording" => "记录",
            "Recording ID" => "记录 ID",
            "Recording uncompressed/compressed size" => "记录未压缩/压缩大小",
            "Recordings" => "记录",
            "Reset chunk browser state" => "重置数据块浏览器状态",
            "Resident" => "常驻",
            "RGB:" => "RGB：",
            "RGBA:" => "RGBA：",
            "Rows" => "行数",
            "Rows (total)" => "行数（总计）",
            "Save" => "保存",
            "Selected blueprint:" => "选中的蓝图：",
            "Selected recording" => "选中的记录",
            "Selectors" => "选择器",
            "Server streaming" => "服务器流",
            "Set RERUN_TRACK_ALLOCATIONS=1 for detailed allocation tracking from startup." => {
                "设置 RERUN_TRACK_ALLOCATIONS=1 以从启动时进行详细分配追踪。"
            },
            "Show all chunks regardless of static/temporal status" => "显示所有数据块，无论静态/时序状态",
            "Show dimension slider" => "显示维度滑块",
            "Show only non-static chunks" => "仅显示非静态数据块",
            "Show only static chunks" => "仅显示静态数据块",
            "Show/hide info and config sections" => "显示/隐藏信息和配置部分",
            "Size" => "大小",
            "Size (total)" => "大小（总计）",
            "Split cleanup" => "拆分清理",
            "Static chunk overwritten by a newer value" => "静态数据块被新值覆盖",
            "Static:" => "静态：",
            "Stats" => "统计",
            "stochastically_tracked:" => "随机追踪：",
            "streaming_counts" => "streaming_counts",
            "streaming_progress" => "streaming_progress",
            "streaming_throughput" => "streaming_throughput",
            "Table stores" => "表存储",
            "Temporal:" => "时序：",
            "Texture Memory:" => "纹理内存：",
            "This timeline is unordered relative to RowId" => "此时间线相对于 RowId 是无序的",
            "This will slow down the program" => "这将降低程序速度",
            "Throughput" => "吞吐量",
            "Timeline:" => "时间线：",
            "To:" => "到：",
            "Toggle info panels" => "切换信息面板",
            "Top memory consumers" => "顶级内存消耗者",
            "Total number of chunks across all pending batches" => "所有待处理批次中的总数据块数",
            "Total:" => "总计：",
            "Transit budget filled" => "传输预算已满",
            "Unsorted" => "未排序",
            "untracked:" => "未追踪：",
            "Used for the book-keeping of the allocation tracker" => "用于分配追踪器的记账",
            "Val:" => "值：",
            "Viewer Caches" => "查看器缓存",
            "VRAM" => "显存",
            "What is the actual size of this cache after deduplication?" => "此缓存去重后的实际大小是多少？",
            "What would be the size of this cache in the worst case, i.e. if all chunks had been fully copied?" => {
                "此缓存的最坏情况大小是多少（即所有数据块都被完全复制）？"
            },
            "width" => "宽度",
            "🗠 Rerun Viewer memory use over time" => "🗠 Rerun 查看器内存使用随时间变化",

            _ => key,
        }
    }
}
