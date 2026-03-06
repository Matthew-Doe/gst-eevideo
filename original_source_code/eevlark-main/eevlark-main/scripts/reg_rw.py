
print(device_path)
init_device(device_path + "/ADSD3500_Dev_User_Device_AD01000_AD01000_232.yaml")


result = read_register("test0_reg")
print("0x%X" % result.value)
print(result.fields)

write_register("test0_reg",{'byte3':0x33})
write_register("test0_reg",{'byte2':0x22})
write_register("test0_reg",{'byte1':0x11}) # produced a 1ms pulse
write_register("test0_reg",{'byte0':0xAA})
read_register("test0_reg")

# write_i2c(
#     port   = "i2c0",
#     i2c_id = MAX_7327_0,
#     addr   = b'',
#     data   = b"\x00"
# )
