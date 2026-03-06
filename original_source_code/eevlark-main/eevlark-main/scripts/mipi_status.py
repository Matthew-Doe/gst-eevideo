def report_mipi_status(port):
  activity = read_register(port+"_Activity")
  print("MIPI port ",port)
  if activity.fields["cactive"]!=1:
    print("  Clock is not active")
  else:
    print("  Clock is active")
    if activity.fields["ccont"]==1:
      print("  Clock is continuous")
    else:
      print("  Clock is not continuous")
    data_types = read_register(port+"_DataType")
    num_lanes = 0
    if activity.fields["l0a"]==1:
      print("  Lane 0 is active")
      num_lanes += 1
    if activity.fields["l1a"]==1:
      print("  Lane 1 is active")
      num_lanes += 1
    if activity.fields["l2a"]==1:
      print("  Lane 2 is active")
      num_lanes += 1
    if activity.fields["l3a"]==1:
      print("  Lane 3 is active")
      num_lanes += 1
    if activity.fields["v0a"]==1:
      print("  Virtual Channel 0 is active")
      bpp = report_data_type(data_types.fields["dtype0"])
    if activity.fields["v1a"]==1:
      print("  Virtual Channel 1 is active")
      bpp = report_data_type(data_types.fields["dtype1"])
    if activity.fields["v2a"]==1:
      print("  Virtual Channel 2 is active")
      bpp = report_data_type(data_types.fields["dtype2"])
    if activity.fields["v3a"]==1:
      print("  Virtual Channel 3 is active")
      bpp = report_data_type(data_types.fields["dtype3"])
    print("  %s bits per pixel" % bpp)
    line_count = read_register(port+"_LineCount")
    print("    %s lines per frame" % line_count.fields["lineCount"])
    line_info = read_register(port+"_LineInfo")
    print("    %s bytes per line" % line_info.fields["bytesPerLine"])
    ppl = line_info.fields["bytesPerLine"]*8/bpp
    print("    %s pixels per line" % ppl)
    # assuming 62.5 MHz system pixel clock
    gbps = line_info.fields["bytesPerLine"]/line_info.fields["pixelClockCount"]*62.5/1000.0/num_lanes*8.0
    print("  %s Gbps per lane" % gbps)

  return

def report_data_type(dtype):
  if dtype<0x10 or dtype>0x3F:
    print("    Unknown DataType 0x%X" % dtype)
  else:
    print("    DataType: 0x%X" % dtype,data_type_names[dtype])
  return data_type_bpp[dtype]


data_type_names = {
  0x10 : "Null",
  0x11 : "Blanking Data",
  0x12 : "Embedded 8-bit non Image Data",
  0x13 : "Reserved",
  0x14 : "Reserved",
  0x15 : "Reserved",
  0x16 : "Reserved",
  0x17 : "Reserved",
  0x18 : "YUV420 8-bit",
  0x19 : "YUV420 10-bit",
  0x1A : "Legacy YUV420 8-bit",
  0x1B : "Reserved",
  0x1C : "YUV420 8-bit (Chroma Shifted Pixel Sampling)",
  0x1D : "YUV420 10-bit (Chroma Shifted Pixel Sampling)",
  0x1E : "YUV422 8-bit",
  0x1F : "YUV422 10-bit",
  0x20 : "RGB444",
  0x21 : "RGB555",
  0x22 : "RGB565",
  0x23 : "RGB666",
  0x24 : "RGB888",
  0x25 : "Reserved",
  0x26 : "Reserved",
  0x27 : "Reserved",
  0x28 : "RAW6",
  0x29 : "RAW7",
  0x2A : "RAW8",
  0x2B : "RAW10",
  0x2C : "RAW12",
  0x2D : "RAW14",
  0x2E : "Reserved",
  0x2F : "Reserved",
  0x30 : "User Defined 8-bit Data Type 1",
  0x31 : "User Defined 8-bit Data Type 2",
  0x32 : "User Defined 8-bit Data Type 3",
  0x33 : "User Defined 8-bit Data Type 4",
  0x34 : "User Defined 8-bit Data Type 5",
  0x35 : "User Defined 8-bit Data Type 6",
  0x36 : "User Defined 8-bit Data Type 7",
  0x37 : "User Defined 8-bit Data Type 8",
  0x38 : "Reserved",
  0x39 : "Reserved",
  0x3A : "Reserved",
  0x3B : "Reserved",
  0x3C : "Reserved",
  0x3D : "Reserved",
  0x3E : "Reserved",
  0x3F : "Reserved",
}

data_type_bpp = {
  0x10 : 8, # "Null",
  0x11 : 8, # "Blanking Data",
  0x12 : 8, # "Embedded 8-bit non Image Data",
  0x13 : 8, # "Reserved",
  0x14 : 8, # "Reserved",
  0x15 : 8, # "Reserved",
  0x16 : 8, # "Reserved",
  0x17 : 8, # "Reserved",
  0x18 : 12, # "YUV420 8-bit",
  0x19 : 15, # "YUV420 10-bit",
  0x1A : 12, # "Legacy YUV420 8-bit",
  0x1B : 8, # "Reserved",
  0x1C : 12, # "YUV420 8-bit (Chroma Shifted Pixel Sampling)",
  0x1D : 15, # "YUV420 10-bit (Chroma Shifted Pixel Sampling)",
  0x1E : 16, # "YUV422 8-bit",
  0x1F : 20, # "YUV422 10-bit",
  0x20 : 12, # "RGB444",
  0x21 : 15, # "RGB555",
  0x22 : 16, # "RGB565",
  0x23 : 18, # "RGB666",
  0x24 : 24, # "RGB888",
  0x25 : 8, # "Reserved",
  0x26 : 8, # "Reserved",
  0x27 : 8, # "Reserved",
  0x28 : 6, # "RAW6",
  0x29 : 7, # "RAW7",
  0x2A : 8, # "RAW8",
  0x2B : 10, # "RAW10",
  0x2C : 12, # "RAW12",
  0x2D : 14, # "RAW14",
  0x2E : 8, # "Reserved",
  0x2F : 8, # "Reserved",
  0x30 : 8, # "User Defined 8-bit Data Type 1",
  0x31 : 8, # "User Defined 8-bit Data Type 2",
  0x32 : 8, # "User Defined 8-bit Data Type 3",
  0x33 : 8, # "User Defined 8-bit Data Type 4",
  0x34 : 8, # "User Defined 8-bit Data Type 5",
  0x35 : 8, # "User Defined 8-bit Data Type 6",
  0x36 : 8, # "User Defined 8-bit Data Type 7",
  0x37 : 8, # "User Defined 8-bit Data Type 8",
  0x38 : 8, # "Reserved",
  0x39 : 8, # "Reserved",
  0x3A : 8, # "Reserved",
  0x3B : 8, # "Reserved",
  0x3C : 8, # "Reserved",
  0x3D : 8, # "Reserved",
  0x3E : 8, # "Reserved",
  0x3F : 8, # "Reserved",
}

