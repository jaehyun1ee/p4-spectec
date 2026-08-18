let sl_schema = "p4spectec.sl.v1"
let value_schema = "p4spectec.value.v1"

let envelope schema kind payload : Yojson.Safe.t =
  `Assoc
    [ ("schema", `String schema); ("kind", `String kind); ("payload", payload) ]

let sl payload = envelope sl_schema "sl" payload
let value payload = envelope value_schema "value" payload
