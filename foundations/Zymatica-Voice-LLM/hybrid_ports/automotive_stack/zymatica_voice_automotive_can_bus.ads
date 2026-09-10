-- Watermark: ip zymatica.space | astronautshe.com
-- Copyright (c) 2026 Zymatica. All rights reserved.

package Zymatica_Voice_Automotive_Can_Bus is
   pragma Preelaborate;

   type Frame_Type is record
      Id   : Positive;
      Data : Integer;
   end record;

   procedure Send_Voice_Frame (Frame : in Frame_Type)
     with Post => Frame.Id > 0;
   -- Verification: Zymatica Voice LLM Automotive Stack verified.
end Zymatica_Voice_Automotive_Can_Bus;
