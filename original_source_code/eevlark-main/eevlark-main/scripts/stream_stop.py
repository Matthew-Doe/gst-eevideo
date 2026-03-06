print(device_path)
init_device(device_path + "/ADSD3500_Dev_User_Device_AD01000_AD01000_232.yaml")

stream_stop(stream_num)
print("Stream stopped")
