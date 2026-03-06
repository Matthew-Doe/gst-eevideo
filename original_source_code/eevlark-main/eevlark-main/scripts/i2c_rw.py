CHIP_ID_REG      = b'\x00\x00'
CHIP_ID          = 0x2481
RESET_REGISTER   = b'\x00\x1A'
MT9M114_ID       = 0x5D

print(device_path)
init_device(device_path + "/ADSD3500_Dev_User_Device_AD01000_AD01000_232.yaml")


write_register("mipi0_GPO",{'en':1})  # Set enable output high

gpo = read_register("mipi0_GPO")
print("GPO",gpo.fields)

write_i2c("i2c0",MT9M114_ID,CHIP_ID_REG,b'')
data = read_i2c("i2c0",MT9M114_ID,2)

print("CHIP_ID_REG = ",", ".join(["0x%X" % b for b in data]))
