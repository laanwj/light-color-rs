import socket
import json
import threading
import time

HOST = '127.0.0.1'
PORT = 4983

# Initial state for 2 lights
lights = [
    {"mode": "cct", "dim": 10, "ct": 3200, "gm": 0, "hue": 0, "sat": 0},
    {"mode": "hsi", "dim": 50, "ct": 5600, "gm": 0, "hue": 120, "sat": 100}
]

def handle_client(conn, addr):
    print(f"Connected by {addr}")
    
    # Send initial state
    response = {"response": "state", "state": lights}
    conn.sendall((json.dumps(response) + "\n").encode('utf-8'))
    
    buffer = ""
    while True:
        data = conn.recv(1024)
        if not data:
            break
        buffer += data.decode('utf-8')
        while "\n" in buffer:
            line, buffer = buffer.split("\n", 1)
            try:
                cmd = json.loads(line)
                print(f"Received: {cmd}")
                
                idx = cmd['idx']
                if idx < len(lights):
                    # Update state
                    new_state = cmd['state']
                    for k, v in new_state.items():
                        if v is not None:
                            lights[idx][k] = v
                
                # Echo state back to all (here just to this client for simplicity, 
                # but real server broadcasts or client polling handles it)
                # The real server sends OK, then potentially state updates if logic changes it.
                # Here we just send OK.
                conn.sendall((json.dumps({"response": "ok"}) + "\n").encode('utf-8'))
                
                # Send updated state back to confirm
                conn.sendall((json.dumps({"response": "state", "state": lights}) + "\n").encode('utf-8'))
                
            except json.JSONDecodeError:
                print("JSON Error")
    
    print(f"Disconnected {addr}")
    conn.close()

def main():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind((HOST, PORT))
        s.listen()
        print(f"Listening on {HOST}:{PORT}")
        while True:
            conn, addr = s.accept()
            t = threading.Thread(target=handle_client, args=(conn, addr))
            t.start()

if __name__ == '__main__':
    main()
