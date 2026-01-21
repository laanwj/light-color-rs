extends TabContainer
class_name LightControl

signal state_changed(state: LCTypes.LightState)

enum Tab {
	CCT = 0,
	HSI = 1,
}

var state: LCTypes.LightState
var process_signals: bool = false

@onready var hsi_control: ColorPicker = $HSI/VBoxContainer/HSIPicker
@onready var dim_control: ValueSlider = $CCT/VBoxContainer/Dim
@onready var ct_control: ValueSlider = $CCT/VBoxContainer/CT
@onready var gm_control: ValueSlider = $CCT/VBoxContainer/GM
@onready var preview1: ColorRect = $HSI/VBoxContainer/ColorRect
@onready var preview2: ColorRect = $CCT/VBoxContainer/ColorRect


func _update_preview() -> void:
	var preview = state.preview
	preview1.color = preview
	preview2.color = preview


func _hsi_changed(color: Color) -> void:
	if not process_signals:
		return
	state.hue = color.h * 360
	state.sat = color.s * 100
	state.dim = color.v * 100
	# make sure dim slider on other tab is synced
	dim_control.value = state.dim
	print("new hsi: ", color)
	_update_preview()
	state_changed.emit(state)


func _dim_changed(value: float) -> void:
	if not process_signals:
		return
	if current_tab == Tab.CCT:
		state.dim = value
		print("new dim: ", value)
		# make sure other tab is synced
		hsi_control.color = Color.from_hsv(state.hue / 360.0, state.sat / 100.0, state.dim / 100.0)
		_update_preview()
		state_changed.emit(state)


func _ct_changed(value: float) -> void:
	if not process_signals:
		return
	state.ct = value
	print("new ct: ", value)
	_update_preview()
	state_changed.emit(state)


func _gm_changed(value: float) -> void:
	if not process_signals:
		return
	state.gm = value
	print("new gm: ", value)
	_update_preview()
	state_changed.emit(state)


func _tab_changed(tab: int) -> void:
	if not process_signals:
		return
	match tab:
		Tab.CCT:
			state.mode = LCTypes.LightMode.CCT
		Tab.HSI:
			state.mode = LCTypes.LightMode.HSI
	print("new mode: ", state.mode)
	_update_preview()
	state_changed.emit(state)


func set_state(new_state: LCTypes.LightState) -> void:
	process_signals = false # inhibit change signals when setting from outside
	state = new_state.copy()
	match state.mode:
		LCTypes.LightMode.CCT:
			current_tab = Tab.CCT
		LCTypes.LightMode.HSI:
			current_tab = Tab.HSI
	hsi_control.color = Color.from_hsv(state.hue / 360.0, state.sat / 100.0, state.dim / 100.0)
	dim_control.value = state.dim
	ct_control.value = state.ct
	gm_control.value = state.gm	
	_update_preview()
	process_signals = true


func _ready() -> void:
	state = LCTypes.LightState.new()
	state.mode = LCTypes.LightMode.CCT
	state.dim = 25
	state.hue = 280
	state.sat = 90
	state.ct = 3500
	state.gm = 0
	set_state(state)

	# connect signals
	tab_changed.connect(_tab_changed)
	hsi_control.color_changed.connect(_hsi_changed)
	dim_control.value_changed.connect(_dim_changed)
	ct_control.value_changed.connect(_ct_changed)
	gm_control.value_changed.connect(_gm_changed)
