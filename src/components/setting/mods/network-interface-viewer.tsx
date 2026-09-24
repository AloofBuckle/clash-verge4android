import { Box } from '@mui/material'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import type { Ref } from 'react'
import { useImperativeHandle, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, DialogRef } from '@/components/base'
import { useNetworkInterfaces } from '@/hooks/use-network'
import { showNotice } from '@/services/notice-service'
import { NetworkInterfaceContent } from './network-interface-content'

export function NetworkInterfaceViewer({ ref }: { ref?: Ref<DialogRef> }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [isV4, setIsV4] = useState(true)

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true)
    },
    close: () => setOpen(false),
  }))

  const { networkInterfaces, loading } = useNetworkInterfaces()
  return (
    <BaseDialog
      open={open}
      title={
        <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
          {t('settings.modals.networkInterface.title')}
          <Box>
            <Button
              variant="contained"
              size="small"
              onClick={() => {
                setIsV4((prev) => !prev)
              }}
            >
              {isV4 ? 'Ipv6' : 'Ipv4'}
            </Button>
          </Box>
        </Box>
      }
      contentSx={{ width: 450 }}
      disableOk
      cancelBtn={t('shared.actions.close')}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
    >
      <NetworkInterfaceContent
        networkInterfaces={networkInterfaces}
        loading={loading}
        isV4={isV4}
        onToggleIpVersion={() => setIsV4((prev) => !prev)}
        onCopy={async (content) => {
          await writeText(content)
          showNotice.success('shared.feedback.notifications.common.copySuccess')
        }}
      />
    </BaseDialog>
  )
}
