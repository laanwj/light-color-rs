/xtends Object # do not instantiate
class_name LCTypes

enum LightMode {
	CCT = 0,
	HSI = 1,
}

const CT_MIN := 2700
const CT_MAX := 7500

const GM_SCALE := 0.2

class LightState:
	var mode: LightMode
	var preview: Color:
		get = _compute_preview
	## Brightness.
	var dim: int = 25
	## Hue (0-360).
	var hue: int = 290
	## Saturation (0-100).
	var sat: int = 100
	## Color temperature (CT_MIN..CT_MAX).
	var ct: int = (CT_MIN + CT_MAX) / 2
	## Green/Magenta (-100..100)
	var gm: int = 0
	
	
	## Return a shallow copy of this object.
	func copy() -> LightState:
		var obj = LightState.new()
		obj.mode = mode
		obj.preview = preview
		obj.dim = dim
		obj.hue = hue
		obj.sat = sat
		obj.ct = ct
		obj.gm = gm
		return obj
	
	
	## Return state object as a plain dictionary, readyto be converted
	## to JSON.
	func to_dict() -> Dictionary:
		var mode_str = ""
		match mode:
			LightMode.CCT:
				mode_str = "cct"
			LightMode.HSI:
				mode_str = "hsi"
			
		return {
			mode = mode_str,
			dim = dim,
			hue = hue,
			sat = sat,
			ct = ct,
			gm = gm,
		}
		

	static func from_dict(d: Dictionary) -> LightState:
		var result = LightState.new()
		if "mode" in d:
			match d["mode"]:
				"cct":
					result.mode = LightMode.CCT
				"hsi":
					result.mode = LightMode.HSI
		if "dim" in d:
			result.dim = d["dim"]
		if "hue" in d:
			result.hue = d["hue"]
		if "sat" in d:
			result.sat = d["sat"]
		if "ct" in d:
			result.ct = d["ct"]
		if "gm" in d:
			result.gm = d["gm"]
		return result
		
		
	func _compute_preview() -> Color:
		var ct_gradient: Gradient = preload("res://ct_gradient.tres")
		var color = Color(0.0, 0.0, 0.0, 1.0)
		match mode:
			LCTypes.LightMode.CCT:
				color = ct_gradient.sample(float(ct - CT_MIN) / (CT_MAX - CT_MIN))
				var blend_color
				if gm < 0.0:
					blend_color = Color(1.0, 0.0, 1.0)
				else:
					blend_color = Color(0.0, 1.0, 0.0)
				blend_color.a = abs(gm) / 100.0 * GM_SCALE
				color = color.blend(blend_color)
			LCTypes.LightMode.HSI:
				color = Color.from_hsv(hue / 360.0, sat / 100.0, 1.0)
		color *= pow(dim / 100.0, 0.25)
		color.a = 1.0
		return color


	func _to_string() -> String:
		return JSON.stringify(to_dict())
	
