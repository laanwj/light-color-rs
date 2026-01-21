extends Node

signal received_lights_state(lights: Array)

const DEFAULT_PORT = 4983

enum State {
	IDLE = 0,
	CONNECTING = 1,
	CONNECTED = 2,
}

var reconnect_timeout := 1.0
var state: State = State.IDLE
var conn: StreamPeerTCP
var linebuf := PackedByteArray()


func set_light(idx: int, lstate: LCTypes.LightState) -> void:
	if state == State.CONNECTED:
		var d := {
			idx=idx,
			state=lstate.to_dict(),
		}
		var s := JSON.stringify(d) + "\n"
		conn.put_data(s.to_utf8_buffer())


func _failed_connect() -> void:
	print("Error connecting to server")
	state = State.IDLE
	# Schedule reconnect
	var timer = get_tree().create_timer(reconnect_timeout, false)
	timer.timeout.connect(_try_connect)


func _try_connect() -> void:
	print("Trying to connect to server")
	state = State.CONNECTING
	linebuf.clear()
	conn = StreamPeerTCP.new()
	var status = conn.connect_to_host("127.0.0.1", DEFAULT_PORT)
	if status != OK:
		_failed_connect()


func _process_line(line: PackedByteArray) -> void:
	var ustr := line.get_string_from_utf8()
	var data := JSON.new()
	data.parse(ustr)
	print("received: ", data.data)
	if "state" in data.data:
		received_lights_state.emit(data.data['state'])

		
func _recv() -> void:
	var avail := conn.get_available_bytes()
	for i in avail:
		var b := conn.get_u8()
		if b == 10:
			_process_line(linebuf)
			linebuf.clear()
		else:
			linebuf.append(b)


func _ready() -> void:
	# Connect to to server
	_try_connect()


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta) -> void:
	if state != State.IDLE:
		conn.poll()
		if state == State.CONNECTING:
			if conn.get_status() == StreamPeerTCP.STATUS_ERROR:
				_failed_connect()
			elif conn.get_status() == StreamPeerTCP.STATUS_CONNECTED:
				print("Succesfully connected")
				state = State.CONNECTED
		elif state == State.CONNECTED:
			if conn.get_status() == StreamPeerTCP.STATUS_CONNECTED:
				_recv()
			else: # conn.get_status() == StreamPeerTCP.STATUS_ERROR or soemthign else unexpected
				print("Error in network connection")
				state = State.IDLE
				_try_connect()
