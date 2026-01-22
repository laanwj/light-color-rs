extends Control

@onready var lc: LightControl = $HBoxContainer/LightControl
@onready var grid: GridContainer = $HBoxContainer/GridContainer
@onready var conn: Node = $NetworkConnection
var num_lights: int = 0
var grid_template_row: Array[Node]
var light_rows: Array
var light_state: Array


## Handler when light control state changed.
func _lc_changed(state: LCTypes.LightState) -> void:
	print("state changed: ", state)
	for row in len(light_rows):
		if light_rows[row][0].button_pressed:
			print("set light ", row, " ", state)
			light_rows[row][1].color = state.preview
			light_state[row] = state.copy()
			conn.set_light(row, light_state[row])


## Handler for events on light's preview swatch.
func _swatch_input(event, idx) -> void:
	if event is InputEventMouseButton and event.button_index == 1 and event.pressed:
		for row in num_lights:
			light_rows[row][0].button_pressed = false
		light_rows[idx][0].button_pressed = true			
		lc.set_state(light_state[idx])
		

## Build table of lights.
func set_num_lights(n: int) -> void:
	if num_lights == n: # Nothing to do?
		return
	num_lights = n
	for child in grid.get_children():
		child.queue_delete()
	for idx in num_lights:
		var row = []
		for child in grid_template_row:
			var new_child = child.duplicate()
			row.append(new_child)
			grid.add_child(new_child)
		row[0].button_pressed = true
		row[1].color = lc.state.preview
		row[1].gui_input.connect(_swatch_input.bind(idx))
		row[2].text = "%d" % [idx]
		light_rows.append(row)
		# XXX read current state from server
		light_state.append(lc.state.copy())


func _recv_lights_state(lights_state_in: Array):
	set_num_lights(len(lights_state_in))
	for idx in len(lights_state_in):
		light_state[idx] = LCTypes.LightState.from_dict(lights_state_in[idx])
		light_rows[idx][1].color = light_state[idx].preview
		
	if len(lights_state_in) != 0:
		# Arbitrarily show color of first light in UI
		lc.set_state(light_state[0])


func _ready() -> void:
	# Disable joystick input for GUI
	var actions := InputMap.get_actions()
	for action in actions:
		var inputs := InputMap.action_get_events(action)
		for input in inputs:
			if input is InputEventJoypadButton or input is InputEventJoypadMotion:
				# -1 seems to be "all devices", 99 is unreasonable
				input.device = 99
	# Handle hiDPI screens
	var dpi: int = DisplayServer.screen_get_dpi()
	print("screen max scale: ", DisplayServer.screen_get_max_scale())
	print("screen DPI: ", dpi)
	if dpi > 192:
		get_window().content_scale_factor = 1.5
		get_window().size = get_window().size * 1.5
	print("scale factor: ", get_window().content_scale_factor)

	# Store and remove template row of grid cells
	grid_template_row = [grid.get_child(0), grid.get_child(1), grid.get_child(2)]
	for child in grid_template_row:
		grid.remove_child(child)

	conn.received_lights_state.connect(_recv_lights_state)
	lc.state_changed.connect(_lc_changed)
