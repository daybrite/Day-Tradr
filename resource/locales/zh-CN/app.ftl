# Day Tradr — 界面文案（简体中文）。

language_name = 简体中文
app_title = Day Tradr
app_tagline = 行情

nav_watchlist = 自选
nav_settings = 设置

watchlist_title = 自选列表
data_mock = 模拟数据 · 确定性样本
data_live = 实时数据
data_attribution = 数据来自 Yahoo Finance — 免费收盘行情。

detail_loading = 正在加载行情…
detail_error = 加载失败：{ $error }

stat_open = 开盘
stat_volume = 成交量
stat_prev_close = 昨收
stat_sma20 = 20日均线
stat_sma50 = 50 日均线
stat_days = 交易日

manage_list_section = 已跟踪代码
manage_add_section = 添加代码
manage_symbol_label = 代码
manage_symbol_hint = Yahoo 代码 — AAPL、SPY、GC=F、EURUSD=X。不区分大小写。
manage_add = 添加
manage_preset_label = 建议
manage_add_preset = 添加所选
manage_remove = 删除
manage_status_added = 已添加
manage_status_removed = 已删除
manage_status_exists = 已在跟踪中
manage_status_empty = 请先输入代码

settings_about_section = 关于
settings_name_label = 名称
settings_version_label = 版本
settings_build_label = 构建日期
settings_website = 基于 Day 构建 — daybrite.dev
settings_data_link = 数据来源 — Yahoo Finance
settings_language_section = 语言
settings_language_label = 语言
settings_system = 系统
settings_theme_section = 外观
settings_theme_label = 主题
theme_light = 浅色
theme_dark = 深色
settings_data_section = 数据
settings_refresh_hint = 从数据源重新获取所有跟踪的代码。
settings_refresh = 全部刷新

open_in_new_window = 在新窗口中打开

# 自选列表概览与排序
breadth_up = 上涨
breadth_down = 下跌
breadth_best = 最佳 { $pct }
breadth_worst = 最差 { $pct }
sort_manual = 自定义
sort_name = 名称
sort_change = 涨跌
watchlist_empty = 未跟踪任何代码
watchlist_empty_hint = 请在“代码”页面添加。

# 图表叠加线与区间条
overlay_label = 均线
overlay_sma20 = 20 日
overlay_sma50 = 50 日
range_day = 当日区间
range_52w = 52 周区间
chip_both = 涨跌额 + %
chip_percent = 百分比
chip_absolute = 涨跌额

# 移动端标签页外壳，以及导航栏 + 按钮的添加流程
nav_symbols = 代码
add_symbol_title = 添加代码
add_symbol_body = 请输入 Yahoo 代码 — AAPL、SPY、GC=F。
menu_add_symbol = 添加代码…
menu_remove_symbol = 删除代码
menu_symbols = 代码

# 行情请求的网络代理
settings_proxy_section = 网络
settings_proxy_label = 代理
settings_proxy_hint = 行情请求经由此处发出。用 %u 标记插入完整 Yahoo 链接的位置，或用 %p 只插入其路径；两者都不含的模板将作为前缀使用。留空则直接请求——网页版需要代理，因为浏览器会拦截对 Yahoo 的跨站请求。
settings_proxy_apply = 应用
settings_proxy_relay = 使用 Daybrite 中继
settings_proxy_direct = 直接请求
