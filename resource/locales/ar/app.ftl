# Day Tradr — نصوص الواجهة (العربية).

language_name = العربية
app_title = Day Tradr
app_tagline = الأسواق

nav_watchlist = المتابعة
nav_settings = الإعدادات

watchlist_title = قائمة المتابعة
data_mock = بيانات تجريبية · قيم حتمية
data_live = بيانات مباشرة
data_attribution = البيانات من Yahoo Finance — أسعار إغلاق مجانية.

detail_loading = جارٍ تحميل الأسعار…
detail_error = تعذر التحميل: { $error }

stat_open = الافتتاح
stat_volume = الحجم
stat_prev_close = الإغلاق السابق
stat_sma20 = متوسط 20
stat_sma50 = متوسط ٥٠
stat_days = الجلسات

manage_list_section = الرموز المتابَعة
manage_add_section = إضافة رمز
manage_symbol_label = الرمز
manage_symbol_hint = رمز Yahoo — AAPL أو SPY أو GC=F أو EURUSD=X. حالة الأحرف لا تهم.
manage_add = إضافة
manage_preset_label = اقتراحات
manage_add_preset = إضافة المحدد
manage_remove = إزالة
manage_status_added = تمت الإضافة
manage_status_removed = تمت الإزالة
manage_status_exists = متابَع بالفعل
manage_status_empty = أدخل رمزًا أولًا

settings_about_section = حول
settings_name_label = الاسم
settings_version_label = الإصدار
settings_build_label = تاريخ البناء
settings_website = بُني بإطار Day — daybrite.dev
settings_data_link = مصدر البيانات — Yahoo Finance
settings_language_section = اللغة
settings_language_label = اللغة
settings_system = النظام
settings_theme_section = المظهر
settings_theme_label = السمة
theme_light = فاتح
theme_dark = داكن
settings_data_section = البيانات
settings_refresh_hint = إعادة جلب كل الرموز المتابَعة من المصدر.
settings_refresh = تحديث الكل

open_in_new_window = فتح في نافذة جديدة

# ملخص قائمة المتابعة وترتيبها
breadth_up = صاعدة
breadth_down = هابطة
breadth_best = الأفضل { $pct }
breadth_worst = الأسوأ { $pct }
sort_manual = مخصص
sort_name = الاسم
sort_change = التغير
watchlist_empty = لا رموز متابَعة
watchlist_empty_hint = أضف رمزًا من صفحة الرموز.

# طبقات الرسم البياني وأشرطة النطاق
overlay_label = المتوسطات
overlay_sma20 = ٢٠ يومًا
overlay_sma50 = ٥٠ يومًا
range_day = نطاق اليوم
range_52w = نطاق ٥٢ أسبوعًا
chip_both = التغير + ٪
chip_percent = نسبة مئوية
chip_absolute = التغير

# واجهة الجوال بعلامات تبويب والإضافة من زر + في الشريط
nav_symbols = الرموز
add_symbol_title = إضافة رمز
add_symbol_body = أدخل رمز Yahoo — AAPL أو SPY أو GC=F.
menu_add_symbol = إضافة رمز…
menu_remove_symbol = حذف الرمز
menu_symbols = الرموز

# وسيط الشبكة لطلبات الأسعار
settings_proxy_section = الشبكة
settings_proxy_label = الوسيط
settings_proxy_hint = تمر طلبات الأسعار عبره. استخدم ‎%u‎ في موضع إدراج رابط Yahoo كاملًا، أو ‎%p‎ لإدراج مساره وحده؛ والقالب الخالي منهما يُعامل كبادئة. اتركه فارغًا للجلب المباشر — نسخة الويب تحتاج وسيطًا لأن المتصفحات تحجب الطلبات عبر المواقع إلى Yahoo.
settings_proxy_apply = تطبيق
settings_proxy_relay = استخدام وسيط Daybrite
settings_proxy_direct = جلب مباشر

# لوحة التحليل أسفل الرسم البياني، وبطاقة الأداء في قائمة المتابعة
analysis_label = التحليل
analysis_drawdown = التراجع
analysis_returns = التحركات اليومية
analysis_monthly = شهري
axis_return = التحرك في جلسة واحدة
performance_title = الأداء، مؤشر إلى ١٠٠

# New interactive charts (day-piece-charts README "Selection").
analysis_profile = ملف الحجم
wl_risk_title = المخاطرة والعائد
wl_corr_title = الارتباط
wl_risk_none = مرِّر أو انقر رمزًا
wl_corr_none = مرِّر أو انقر خلية
wl_risk_readout = ‏{ $symbol }: عائد { $ret } عند تقلب { $vol }
