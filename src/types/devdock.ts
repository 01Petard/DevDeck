export type IpVersion = 4 | 6 | null;

export interface PortListener {
  pid: number;
  processName: string;
  processStartTime: number | null;
  executablePath: string | null;
  protocol: "TCP";
  address: string;
  port: number;
  ipVersion: IpVersion;
}
