package eev

import (
	"fmt"
	"context"
	"time"
)

type I2cRxDataType struct {
	Ack    uint32
	Data   uint32
}

func i2cWriteDeviceID(port string,addr uint32,read uint32) error {

	i2cID := (addr<<1)+read

	if Verbose>0 {fmt.Printf("Writing Device Address to %s\n",port+"_TxFIFO")}
	err := Device.WriteRegFields(port+"_TxFIFO",map[string]uint32{"stop":0,"hold":1,"ack":1,"data":i2cID}) // 0 for write
  if err!=nil {
  	return fmt.Errorf("Error EEV i2cWriteDeviceID failed writing address: \n  %w",err)
  }
  devAddrRsp, err := i2cWaitForRx(port, 1)
  if err!=nil {
  	return fmt.Errorf("Error EEV i2cWriteDeviceID Unable to retrieve Rx FIFO Byte \n  %w",err)
  }
  if (devAddrRsp[0].Ack=='1' || devAddrRsp[0].Data!=i2cID) {
  	if Verbose>0 {
  		fmt.Printf("Info EEV after i2cWriteDeviceID resetting I2C Controller Ack=%d ID=0x%X\n",devAddrRsp[0].Ack,devAddrRsp[0].Data)
  	}
		Device.WriteRegFields(port+"_Config",map[string]uint32{"reset":1}) // Reset Controller
  	return fmt.Errorf("Error EEV Bad I2C device ID response ack=%d(0) ID=0x%X(0x%X)\n",devAddrRsp[0].Ack,devAddrRsp[0].Data,i2cID)
  }

  return nil
}

func i2cWriteBytes(port string, wrBytes []byte, endStop uint32, endHold uint32) error {
	var stop uint32
	var hold uint32
	for index, byte := range wrBytes {
		if index==(len(wrBytes)-1) {  // Last Byte
			stop = endStop;
			hold = endHold
		} else {
			stop = 0
			hold = 1
		}
	  err := Device.WriteRegFields(port+"_TxFIFO",map[string]uint32{"stop":stop,"hold":hold,"ack":1,"data":uint32(byte)}) // 0 for write
	  if err!=nil {
	  	return fmt.Errorf("I2c byte write failed: \n%w",err)
	  }
	}
  wrRsp, err := i2cWaitForRx(port, len(wrBytes))
  if err!=nil {
  	return fmt.Errorf("Unable to retrieve Rx FIFO Bytes\n")
  }
  for index, rspByte := range wrRsp {
  	if rspByte.Ack==1 {
  		if Verbose>2 {fmt.Printf("  Byte %2d Bad Tx Ack, expecting 0\n",index)}
    } else if byte(rspByte.Data)!=wrBytes[index] {
    	if Verbose>2 {fmt.Printf("  Byte %2d Bad Tx Data, expecting %02X, got %02X\n",index,wrBytes[index],rspByte.Data)}
    } else {
    	if Verbose>2 {fmt.Printf("  Byte %2d Ack and Data Correct %02X\n",index,rspByte.Data)}
    }
	}
	return nil
}

func i2cReadBytes(port string, rdLen int) ([]byte,error) {
	var stop uint32
	var hold uint32
	var ack uint32
	for x := 0;x<rdLen;x++ {
		if x==rdLen-1 {
			stop = 1
			hold = 0
			ack = 1
		} else {
			stop = 0
			hold = 1
			ack = 0
		}
  	fields := map[string]uint32{"stop":stop,"hold":hold,"ack":ack,"data":0xFF}
  	err := Device.WriteRegFields(port+"_TxFIFO",fields) // 0 for write
	  if err!=nil {
		  return []byte(nil), fmt.Errorf("I2C TxFIFO WriteRegFields failed: \n%w",err)
	  }
	}

	readData,err := i2cWaitForRx(port,rdLen)
	if err!=nil {
		return []byte(nil),fmt.Errorf("Error getting Rx Fifo return bytes: \n%w", err)
	}

	rxBytes := make([]byte,rdLen)
	for index, returnByte := range readData {
		if returnByte.Ack=='1' {
			return []byte(nil),fmt.Errorf("Back Ack(1) for byte %v\n",index)
		}
		rxBytes[index] = byte(returnByte.Data)
	}

	return rxBytes, nil
}

func i2cWaitForRx(port string, count int) ([]I2cRxDataType, error) {

  ctx, cancel := context.WithTimeout(context.Background(), 100 * time.Millisecond)
  defer cancel() // always good practice

  rxData    := make([]I2cRxDataType,count);
  rxDataCnt := 0;
  attempt    := 0;

  for {
    // (executes at least once)
    attempt++

    statusFields, _ := i2cReadStatus(port)
    if Verbose>1 {fmt.Printf("  Attempt #%d %v\n", attempt,statusFields)}
    if statusFields["rxlevel"] > 0 {
    	for x := 0; x < int(statusFields["rxlevel"]) && rxDataCnt<count; x++ {
      	_, fields, _ := Device.ReadReg(port+"_RxFIFO")
      	rxData[rxDataCnt].Ack  = fields["ack"]
      	rxData[rxDataCnt].Data = fields["data"]
      	rxDataCnt++
      	err := Device.WriteRegFields(port+"_RxFIFO",map[string]uint32{"ack":1}) // Advance FIFO
      	if (err!=nil) {
      		return nil, fmt.Errorf("Error EEV i2cWaitfor RxFIFO Write \n  %w",err)
      	}
      }
    } else {
      time.Sleep(10 * time.Millisecond)
    }

    if rxDataCnt == count {
        if Verbose>1 {fmt.Printf("  Found %d Rx Bytes\n",count)}
        break
    }

    // Check timeout after each iteration
    if ctx.Err() != nil {
        return nil, fmt.Errorf("Timed out waiting for I2C Rx bytes %w",ctx.Err())
    }
  }
  return rxData, nil
}

func i2cReadStatus(port string) (map[string]uint32, error) {
	_, fields, _ := Device.ReadReg(port+"_Status")
	return fields, nil
}

func WriteI2C(port string, i2cID uint32, addrBytes[]byte, dataBytes []byte) error {
	var err error
	wrBytes := append(addrBytes,dataBytes...)

	if Verbose>0 {fmt.Printf("I2C Write to %s 0x%X\n",port,i2cID)}
  err = i2cWriteDeviceID(port,i2cID,0)
  if err!=nil {
  	return fmt.Errorf("Device Address write Failed %w",err)
  } else {
  	if Verbose>2 {fmt.Println("  Device address write ack=0")}
  }

  err = i2cWriteBytes(port,wrBytes,1,0)
  if err!=nil {
  	return fmt.Errorf("  Byte Write Failed : %w\n")
  }

	return nil
}

func ReadI2c(port string, i2cID uint32, rdLen int) ([]byte,error) {
	fmt.Printf("Read Only Transaction to 0x%02X, %d bytes\n",i2cID,rdLen)
	err := i2cWriteDeviceID(port,i2cID,1)
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2cAddr-WriteID/read Failed:\n  %w",err)
  }

  rxBytes, err := i2cReadBytes(port,rdLen)
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2c-ReadBytes Failed:\n  %w",err)
  }

	return rxBytes, nil
}


func ReadI2cAddr(port string, i2cID uint32, addrBytes []byte, rdLen int) ([]byte,error) {
	if Verbose>0 {fmt.Printf("Write Address Then Read Transaction to 0x%02X @0x%X %d bytes\n",i2cID,addrBytes,rdLen)}
	// Make sure the Controller is Idle
	statusFields, _ := i2cReadStatus(port)
	if statusFields["txlevel"]!=0 || statusFields["done"]==0 {
		return []byte(nil), fmt.Errorf("Error EEV I2C Controller %s is busy TxLevel=%v Done=%v \n",port,statusFields["txlevel"],statusFields["done"])
	} else if statusFields["rxLevel"]!=0 || statusFields["holding"]==1 {
		fmt.Printf("Info EEV ReadI2cAddr resetting I2C Controller RxLevel=%d holding=%d\n",statusFields["rxLevel"],statusFields["holding"])
		err := Device.WriteRegFields(port+"_Config",map[string]uint32{"reset":1}) // Reset Controller
		if err!=nil {
			return []byte(nil), fmt.Errorf("Error EEV ReadI2cAddr Reset I2C Failed: \n  %w",err)
		}
	}

	err := i2cWriteDeviceID(port,i2cID,0)  // first access for write
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2cAddr-WriteID/write Failed:\n  %w",err)
  }

  err = i2cWriteBytes(port, addrBytes,0,0) // set up for repeated start
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2cAddr-WriteAddrBytes Failed:\n  %w",err)
  }

	err = i2cWriteDeviceID(port,i2cID,1)  // second access for read
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2cAddr-WriteID/read Failed:\n  %w",err)
  }

  rxBytes, err := i2cReadBytes(port, rdLen)
  if err!=nil {
  	return []byte(nil), fmt.Errorf("ReadI2cAddr-ReadBytes Failed :\n  %w",err)
  }

	return rxBytes, nil
}

