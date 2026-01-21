extends HBoxContainer
class_name ValueSlider

@export var min_value := 0.0
@export var max_value := 100.0
@export var texture: Texture2D

@onready var slider: Slider = $Slider
@onready var spinbox: SpinBox = $Value

signal value_changed(value: float)

var value: int:
	set(new_value):
		spinbox.value = new_value
	get:
		return spinbox.value


func _drag_ended(_value_changed: bool) -> void:
	spinbox.value = slider.value


func _spinbox_value_changed(value: float) -> void:
	slider.value = value
	value_changed.emit(value)


func _draw_slider() -> void:
	var points := PackedVector2Array()
	var colors := PackedColorArray()
	var margin: float = 16 * get_theme_default_base_scale()
	
	slider.draw_texture_rect(texture, Rect2(0, 0, slider.size.x, margin), false)
	#var left_color := Color(0.0, 0.0, 0.0, 1.0)
	#var right_color := Color(1.0, 1.0, 1.0, 1.0)

	#points.append(Vector2(0.0, 0.0))
	#points.append(Vector2(slider.size.x, 0.0))
	#points.append(Vector2(slider.size.x, margin))
	#points.append(Vector2(0.0, margin))
	
	#colors.append(left_color)
	#colors.append(right_color)
	#colors.append(right_color)
	#colors.append(left_color)

	#slider.draw_polygon(points, colors)

func _ready() -> void:
	slider.min_value = min_value
	slider.max_value = max_value
	slider.add_theme_icon_override("grabber", get_theme_icon("bar_arrow", "ColorPicker"))
	slider.add_theme_icon_override("grabber_highlight", get_theme_icon("bar_arrow", "ColorPicker"))
	slider.add_theme_constant_override("grabber_offset", 8 * get_theme_default_base_scale())
	slider.add_theme_stylebox_override("slider", StyleBoxEmpty.new())
	slider.connect("draw", _draw_slider)
	slider.drag_ended.connect(_drag_ended)
	
	spinbox.min_value = min_value
	spinbox.max_value = max_value
	spinbox.value_changed.connect(_spinbox_value_changed)

