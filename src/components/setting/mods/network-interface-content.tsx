import { ContentCopyRounded } from '@mui/icons-material'
import { alpha, Box, Button, CircularProgress, IconButton } from '@mui/material'

import { BaseEmpty } from '@/components/base/base-empty'

export interface NetworkInterfaceAddressView {
  V4?: { ip: string } | null
  V6?: { ip: string } | null
}

export interface NetworkInterfaceView {
  name: string
  addr: NetworkInterfaceAddressView[]
  mac_addr?: string | null
}

export function NetworkInterfaceContent({
  networkInterfaces,
  loading,
  isV4,
  onToggleIpVersion,
  onCopy,
}: {
  networkInterfaces: NetworkInterfaceView[]
  loading: boolean
  isV4: boolean
  onToggleIpVersion: () => void
  onCopy: (content: string) => void | Promise<void>
}) {
  const isEmpty = networkInterfaces.length === 0
  const getAddressIp = (address: NetworkInterfaceAddressView) =>
    isV4 ? address.V4?.ip : address.V6?.ip

  return (
    <>
      <Box sx={{ display: 'flex', justifyContent: 'flex-end', mb: 1 }}>
        <Button variant="contained" size="small" onClick={onToggleIpVersion}>
          {isV4 ? 'Ipv6' : 'Ipv4'}
        </Button>
      </Box>
      {loading && isEmpty ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 4 }}>
          <CircularProgress size={24} />
        </Box>
      ) : isEmpty ? (
        <Box sx={{ minHeight: 160 }}>
          <BaseEmpty />
        </Box>
      ) : (
        networkInterfaces.map((item) => (
          <Box key={item.name}>
            <h4>{item.name}</h4>
            <Box>
              {item.addr.map((address) => {
                const ip = getAddressIp(address)
                return (
                  ip && (
                    <AddressDisplay
                      key={ip}
                      label="IP Address"
                      content={ip}
                      onCopy={onCopy}
                    />
                  )
                )
              })}
              <AddressDisplay
                label="MAC Address"
                content={item.mac_addr ?? ''}
                onCopy={onCopy}
              />
            </Box>
          </Box>
        ))
      )}
    </>
  )
}

function AddressDisplay({
  label,
  content,
  onCopy,
}: {
  label: string
  content: string
  onCopy: (content: string) => void | Promise<void>
}) {
  return (
    <Box
      sx={{
        display: 'flex',
        justifyContent: 'space-between',
        margin: '8px 0',
      }}
    >
      <Box>{label}</Box>
      <Box
        sx={({ palette }) => ({
          borderRadius: '8px',
          padding: '2px 2px 2px 8px',
          background:
            palette.mode === 'dark'
              ? alpha(palette.background.paper, 0.3)
              : alpha(palette.grey[400], 0.3),
        })}
      >
        <Box sx={{ display: 'inline', userSelect: 'text' }}>{content}</Box>
        <IconButton size="small" onClick={() => void onCopy(content)}>
          <ContentCopyRounded sx={{ fontSize: '18px' }} />
        </IconButton>
      </Box>
    </Box>
  )
}
