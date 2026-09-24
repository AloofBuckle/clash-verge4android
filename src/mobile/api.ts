import { invoke, isTauri } from '@tauri-apps/api/core'

export interface Profile {
  id: string
  name: string
  source: string | null
  yaml: string
  updatedAt: number
  selected: { group: string; member: string }[]
  proxyChain: { targetGroup: string; nodes: string[] } | null
  option: ProfileOptions
  mergeYaml?: string | null
  rulesYaml?: string | null
  proxiesYaml?: string | null
  groupsYaml?: string | null
  scriptJs?: string | null
}
export interface ProfileOptions {
  userAgent?: string | null
  withProxy?: boolean | null
  selfProxy?: boolean | null
  updateInterval?: number | null
  timeoutSeconds?: number | null
  dangerAcceptInvalidCerts?: boolean | null
  allowAutoUpdate?: boolean | null
}
export interface ProfileDocument {
  activeId: string | null
  profiles: Profile[]
  globalMergeYaml?: string | null
  globalScriptJs?: string | null
}
export interface Capabilities {
  profileManagement: boolean
  rootProbe: boolean
  tunControl: boolean
  systemProxy: boolean
  liveCore: boolean
  stage: string
}
export interface Summary {
  nodes: { name: string; protocol: string }[]
  groups: {
    name: string
    kind: string
    members: string[]
    providers: string[]
  }[]
  rules: string[]
  mode: string
  providerCount: number
}
export interface RootProbe {
  granted: boolean
  detail: string
}
export interface AgentStatus {
  version: string
  agentPid: number
  coreRunning: boolean
  corePid: number | null
  coreInstalled: boolean
  configPresent: boolean
  geositePresent: boolean
  controllerSocket: string
  transparentActive: boolean
}
export interface RuntimeStatus {
  packaged: boolean
  agentConnected: boolean
  coreRunning: boolean
  coreConnected: boolean
  transparentActive: boolean
  agent: AgentStatus | null
  detail: string
}
export interface SystemProxyStatus {
  supported: boolean
  permissionGranted: boolean
  active: boolean
  phase: string
  detail: string
  bridgeAbi: number
}
export interface BootModuleStatus {
  installed: boolean
  detail: string
}
export interface CoreUpgradeReport {
  upgraded: boolean
  from: string
  to: string
}
export interface LocalBackupInfo {
  filename: string
  createdAt: number
  profileCount: number
}
export interface BackupSettings {
  webdavUrl: string
  webdavUsername: string
  webdavPassword: string
  webdavAcceptInvalidCerts: boolean
  autoScheduleEnabled: boolean
  autoIntervalHours: number
  autoOnChange: boolean
  lastAutoBackupAt: number | null
}
export interface WebDavBackupInfo {
  filename: string
  lastModified: string
  contentLength: number
}
export interface DnsOverrideSettings {
  enabled: boolean
  yaml: string
}
export interface PortSettings {
  allowLan: boolean
  bindAddress: string
  mixedPort: number
  httpPort: number
  socksPort: number
  redirPort: number
  tproxyPort: number
}
export interface ExternalControllerSettings {
  enabled: boolean
  address: string
  secret: string
  allowPrivateNetwork: boolean
  allowOrigins: string[]
}
export interface NetworkInterfaceInfo {
  name: string
  addr: Array<{
    V4?: { ip: string; broadcast?: string | null; netmask?: string | null }
    V6?: { ip: string; broadcast?: string | null; netmask?: string | null }
  }>
  mac_addr?: string | null
  index: number
  internal: boolean
}
export interface CoreGroup {
  name: string
  kind: string
  selected: string | null
  members: string[]
  selectable: boolean
}
export interface CoreRule {
  kind: string
  payload: string
  proxy: string
}
export interface CoreSnapshot {
  version: string
  mode: string
  groups: CoreGroup[]
  rules: CoreRule[]
}
export interface CoreConnection {
  id: string
  network: string
  inbound: string
  source: string
  destination: string
  host: string
  process: string
  rule: string
  rulePayload: string
  chains: string[]
  upload: number
  download: number
  start: string
}
export interface ConnectionsSnapshot {
  downloadTotal: number
  uploadTotal: number
  memory: number
  connections: CoreConnection[]
}
export interface SubscriptionInfo {
  upload: number
  download: number
  total: number
  expire: number
}
export interface ProxyProviderSummary {
  name: string
  vehicleType: 'HTTP' | 'File' | 'Inline' | string
  updatedAt: string | null
  subscriptionInfo: SubscriptionInfo | null
  proxyCount: number
}
export interface RuleProviderSummary {
  name: string
  vehicleType: 'HTTP' | 'File' | 'Inline' | string
  behavior: string
  ruleCount: number
  updatedAt: string | null
}
export interface RuntimePreferences {
  ipv6: boolean
  unifiedDelay: boolean
  logLevel: 'debug' | 'info' | 'warning' | 'warn' | 'error' | 'silent' | string
  tcpConcurrent: boolean
  findProcessMode: 'off' | 'strict' | 'always' | string
}
export interface MobilePreferences {
  autoCloseConnection: boolean
  defaultLatencyTest: string
  defaultLatencyTimeout: number
  enableAutoDelayDetection: boolean
  autoDelayDetectionIntervalMinutes: number
  startPage:
    | 'home'
    | 'proxies'
    | 'profiles'
    | 'connections'
    | 'rules'
    | 'logs'
    | 'settings'
}
export interface TunPreferences {
  stack: 'system' | 'gvisor' | 'mixed' | 'mips' | string
  strictRoute: boolean
  autoDetectInterface: boolean
  dnsHijack: string[]
  mtu: number
}

export const unavailable: Capabilities = {
  profileManagement: false,
  rootProbe: false,
  tunControl: false,
  systemProxy: false,
  liveCore: false,
  stage: 'disconnected',
}
export const hasBackend = () => isTauri()
export const api = {
  capabilities: () => invoke<Capabilities>('mobile_capabilities'),
  profiles: () => invoke<ProfileDocument>('mobile_profiles'),
  inspect: (yaml: string) => invoke<Summary>('mobile_inspect', { yaml }),
  import: (name: string, yaml: string) =>
    invoke<ProfileDocument>('mobile_import', { name, yaml }),
  importNodes: (name: string, links: string) =>
    invoke<ProfileDocument>('mobile_import_nodes', { name, links }),
  subscribe: (name: string, url: string, option?: ProfileOptions) =>
    invoke<ProfileDocument>('mobile_subscribe', { name, url, option }),
  activate: (id: string) => invoke<ProfileDocument>('mobile_activate', { id }),
  remove: (id: string) => invoke<ProfileDocument>('mobile_delete', { id }),
  removeMany: (ids: string[]) =>
    invoke<ProfileDocument>('mobile_delete_many', { ids }),
  updateProfile: (
    id: string,
    name: string,
    yaml: string,
    expectedYaml: string,
    source?: string | null,
    option?: ProfileOptions,
  ) =>
    invoke<ProfileDocument>('mobile_update_profile', {
      id,
      name,
      yaml,
      expectedYaml,
      source,
      option,
    }),
  reorderProfile: (activeId: string, overId: string) =>
    invoke<ProfileDocument>('mobile_reorder_profile', { activeId, overId }),
  setGlobalMerge: (yaml?: string | null) =>
    invoke<ProfileDocument>('mobile_set_global_merge', { yaml: yaml ?? null }),
  setProfileMerge: (id: string, yaml?: string | null) =>
    invoke<ProfileDocument>('mobile_set_profile_merge', {
      id,
      yaml: yaml ?? null,
    }),
  setProfileSequence: (
    id: string,
    kind: 'rules' | 'proxies' | 'groups',
    yaml?: string | null,
  ) =>
    invoke<ProfileDocument>('mobile_set_profile_sequence', {
      id,
      kind,
      yaml: yaml ?? null,
    }),
  setGlobalScript: (script?: string | null) =>
    invoke<ProfileDocument>('mobile_set_global_script', {
      script: script ?? null,
    }),
  setProfileScript: (id: string, script?: string | null) =>
    invoke<ProfileDocument>('mobile_set_profile_script', {
      id,
      script: script ?? null,
    }),
  refresh: (id: string) => invoke<ProfileDocument>('mobile_refresh', { id }),
  nextUpdateTime: (id: string) =>
    invoke<number | null>('mobile_next_update_time', { id }),
  probeRoot: () => invoke<RootProbe>('mobile_probe_root'),
  runtimeStatus: () => invoke<RuntimeStatus>('mobile_runtime_status'),
  systemProxyStatus: () =>
    invoke<SystemProxyStatus>('mobile_system_proxy_status'),
  setSystemProxy: (enable: boolean) =>
    invoke<SystemProxyStatus>('mobile_set_system_proxy', { enable }),
  bootModuleStatus: () => invoke<BootModuleStatus>('mobile_boot_module_status'),
  setBootModule: (enable: boolean) =>
    invoke<BootModuleStatus>('mobile_set_boot_module', { enable }),
  runtimeConfig: () => invoke<string>('mobile_runtime_config'),
  bootstrapRuntime: () => invoke<RuntimeStatus>('mobile_bootstrap_runtime'),
  startPreviewCore: () => invoke<CoreSnapshot>('mobile_start_preview_core'),
  stopPreviewCore: () => invoke<RuntimeStatus>('mobile_stop_preview_core'),
  restartCore: () => invoke<CoreSnapshot>('mobile_restart_core'),
  upgradeCore: (force = false) =>
    invoke<CoreUpgradeReport>('mobile_core_upgrade', { force }),
  createLocalBackup: () =>
    invoke<LocalBackupInfo>('mobile_create_local_backup'),
  listLocalBackups: () =>
    invoke<LocalBackupInfo[]>('mobile_list_local_backups'),
  restoreLocalBackup: (filename: string) =>
    invoke<ProfileDocument>('mobile_restore_local_backup', { filename }),
  deleteLocalBackup: (filename: string) =>
    invoke<void>('mobile_delete_local_backup', { filename }),
  exportLocalBackup: (filename: string) =>
    invoke<string>('mobile_export_local_backup', { filename }),
  importBackupText: (text: string) =>
    invoke<LocalBackupInfo>('mobile_import_backup_text', { text }),
  backupSettings: () => invoke<BackupSettings>('mobile_backup_settings'),
  setBackupSettings: (settings: BackupSettings) =>
    invoke<BackupSettings>('mobile_set_backup_settings', { settings }),
  testWebDav: () => invoke<void>('mobile_test_webdav'),
  createWebDavBackup: () =>
    invoke<WebDavBackupInfo>('mobile_create_webdav_backup'),
  listWebDavBackups: () =>
    invoke<WebDavBackupInfo[]>('mobile_list_webdav_backups'),
  restoreWebDavBackup: (filename: string) =>
    invoke<ProfileDocument>('mobile_restore_webdav_backup', { filename }),
  deleteWebDavBackup: (filename: string) =>
    invoke<void>('mobile_delete_webdav_backup', { filename }),
  coreSnapshot: () => invoke<CoreSnapshot>('mobile_core_snapshot'),
  runtimePreferences: () =>
    invoke<RuntimePreferences>('mobile_runtime_preferences'),
  setRuntimePreferences: (preferences: RuntimePreferences) =>
    invoke<RuntimePreferences>('mobile_set_runtime_preferences', {
      preferences,
    }),
  preferences: () => invoke<MobilePreferences>('mobile_preferences'),
  setPreferences: (preferences: MobilePreferences) =>
    invoke<MobilePreferences>('mobile_set_preferences', { preferences }),
  dnsOverride: () => invoke<DnsOverrideSettings>('mobile_dns_override'),
  setDnsOverride: (settings: DnsOverrideSettings) =>
    invoke<DnsOverrideSettings>('mobile_set_dns_override', { settings }),
  portSettings: () => invoke<PortSettings>('mobile_port_settings'),
  setPortSettings: (settings: PortSettings) =>
    invoke<PortSettings>('mobile_set_port_settings', { settings }),
  externalControllerSettings: () =>
    invoke<ExternalControllerSettings>('mobile_external_controller_settings'),
  setExternalControllerSettings: (settings: ExternalControllerSettings) =>
    invoke<ExternalControllerSettings>(
      'mobile_set_external_controller_settings',
      { settings },
    ),
  networkInterfaces: () =>
    invoke<NetworkInterfaceInfo[]>('mobile_network_interfaces'),
  tunPreferences: () => invoke<TunPreferences>('mobile_tun_preferences'),
  setTunPreferences: (preferences: TunPreferences) =>
    invoke<TunPreferences>('mobile_set_tun_preferences', { preferences }),
  selectProxy: (group: string, member: string) =>
    invoke<CoreSnapshot>('mobile_select_proxy', { group, member }),
  setProxyChain: (targetGroup: string, nodes: string[]) =>
    invoke<CoreSnapshot>('mobile_set_proxy_chain', { targetGroup, nodes }),
  clearProxyChain: () => invoke<CoreSnapshot>('mobile_clear_proxy_chain'),
  setMode: (mode: 'rule' | 'global' | 'direct') =>
    invoke<CoreSnapshot>('mobile_set_mode', { mode }),
  connections: () => invoke<ConnectionsSnapshot>('mobile_connections'),
  closeConnection: (id: string) =>
    invoke<ConnectionsSnapshot>('mobile_close_connection', { id }),
  closeAllConnections: () =>
    invoke<ConnectionsSnapshot>('mobile_close_all_connections'),
  proxyDelay: (proxy: string) =>
    invoke<number>('mobile_proxy_delay', { proxy }),
  proxyProviders: () =>
    invoke<ProxyProviderSummary[]>('mobile_proxy_providers'),
  updateProxyProvider: (name: string) =>
    invoke<ProxyProviderSummary[]>('mobile_update_proxy_provider', { name }),
  ruleProviders: () =>
    invoke<RuleProviderSummary[]>('mobile_rule_providers'),
  updateRuleProvider: (name: string) =>
    invoke<RuleProviderSummary[]>('mobile_update_rule_provider', { name }),
  flushFakeIp: () => invoke<void>('mobile_flush_fakeip'),
  flushDns: () => invoke<void>('mobile_flush_dns'),
  updateGeo: () => invoke<void>('mobile_update_geo'),
  diagnostics: () => invoke<string>('mobile_diagnostics'),
  logs: (lines = 200) => invoke<string>('mobile_logs', { lines }),
  clearLogs: () => invoke<void>('mobile_clear_logs'),
  enableTun: () => invoke<CoreSnapshot>('mobile_enable_tun'),
  disableTun: () => invoke<RuntimeStatus>('mobile_disable_tun'),
}
