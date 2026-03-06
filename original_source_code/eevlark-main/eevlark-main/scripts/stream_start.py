print(device_path)
init_device(device_path + "/ADSD3500_Dev_User_Device_AD01000_AD01000_232.yaml")

dest_port = 0
delay = 1200
max_pkt_size = 1000

stream_start(stream_num, dest_port, delay, max_pkt_size)
print("Stream started")
