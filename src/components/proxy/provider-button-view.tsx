import { RefreshRounded, StorageOutlined } from '@mui/icons-material'
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  LinearProgress,
  List,
  ListItem,
  ListItemText,
  Typography,
  alpha,
  styled,
} from '@mui/material'
import dayjs from 'dayjs'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import parseTraffic from '@/utils/parse-traffic'

export interface ProxyProviderButtonItem {
  name: string
  vehicleType: string
  proxyCount: number
  updatedAt?: string | null
  subscriptionInfo?: {
    upload: number
    download: number
    total: number
    expire: number
  } | null
}

interface Props {
  providers: ProxyProviderButtonItem[]
  providerUnavailable?: boolean
  updating: Record<string, boolean>
  onUpdate: (name: string) => void | Promise<void>
  onUpdateAll: () => void | Promise<void>
}

const TypeBox = styled(Box)<{ component?: React.ElementType }>(({ theme }) => ({
  display: 'inline-block',
  border: '1px solid #ccc',
  borderColor: alpha(theme.palette.secondary.main, 0.5),
  color: alpha(theme.palette.secondary.main, 0.8),
  borderRadius: 4,
  fontSize: 10,
  marginRight: '4px',
  padding: '0 2px',
  lineHeight: 1.25,
}))

const parseExpire = (expire?: number) => {
  if (!expire) return '-'
  return dayjs(expire * 1000).format('YYYY-MM-DD')
}

export const ProxyProviderButtonView = ({
  providers,
  providerUnavailable = false,
  updating,
  onUpdate,
  onUpdateAll,
}: Props) => {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)

  if (providers.length === 0 && !providerUnavailable) return null

  return (
    <>
      <Button
        variant="outlined"
        size="small"
        startIcon={<StorageOutlined />}
        onClick={() => setOpen(true)}
        disabled={providerUnavailable}
        color={providerUnavailable ? 'warning' : 'primary'}
        sx={{
          mr: 1,
          ...(providerUnavailable && {
            '&.Mui-disabled': {
              color: 'warning.main',
              borderColor: 'warning.main',
            },
          }),
        }}
      >
        {t('proxies.page.provider.title')}
      </Button>

      <Dialog open={open} onClose={() => setOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>
          <Box
            sx={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <Typography variant="h6">
              {t('proxies.page.provider.title')}
            </Typography>
            <Box>
              <Button
                variant="contained"
                size="small"
                onClick={() => void onUpdateAll()}
                aria-label={t('proxies.page.provider.actions.updateAll')}
              >
                {t('proxies.page.provider.actions.updateAll')}
              </Button>
            </Box>
          </Box>
        </DialogTitle>

        <DialogContent>
          <List sx={{ py: 0, minHeight: 250 }}>
            {providers.map((provider) => {
              const key = provider.name
              const updatedAt = provider.updatedAt
                ? dayjs(provider.updatedAt)
                : null
              const isUpdating = updating[key]
              const sub = provider.subscriptionInfo
              const upload = sub?.upload || 0
              const download = sub?.download || 0
              const total = sub?.total || 0
              const expire = sub?.expire || 0
              const progress =
                total > 0
                  ? Math.min(
                      Math.round(((download + upload) * 100) / total) + 1,
                      100,
                    )
                  : 0

              return (
                <ListItem
                  key={key}
                  sx={[
                    {
                      p: 0,
                      mb: '8px',
                      borderRadius: 2,
                      overflow: 'hidden',
                      transition: 'all 0.2s',
                    },
                    ({ palette: { mode, primary } }) => {
                      const bgcolor = mode === 'light' ? '#ffffff' : '#24252f'
                      const hoverColor =
                        mode === 'light'
                          ? alpha(primary.main, 0.1)
                          : alpha(primary.main, 0.2)
                      return {
                        backgroundColor: bgcolor,
                        '&:hover': { backgroundColor: hoverColor },
                      }
                    },
                  ]}
                >
                  <ListItemText
                    sx={{ px: 2, py: 1 }}
                    primary={
                      <Box
                        sx={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                        }}
                      >
                        <Typography
                          variant="subtitle1"
                          component="div"
                          noWrap
                          title={key}
                          sx={{ display: 'flex', alignItems: 'center' }}
                        >
                          <span style={{ marginRight: '8px' }}>{key}</span>
                          <TypeBox component="span">{provider.proxyCount}</TypeBox>
                          <TypeBox component="span">{provider.vehicleType}</TypeBox>
                        </Typography>
                        <Typography variant="body2" color="text.secondary" noWrap>
                          <small>{t('shared.labels.updateAt')}: </small>
                          {updatedAt?.fromNow() ?? '-'}
                        </Typography>
                      </Box>
                    }
                    secondary={
                      sub ? (
                        <>
                          <Box
                            sx={{
                              mb: 1,
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'space-between',
                            }}
                          >
                            <span title={t('shared.labels.usedTotal') as string}>
                              {parseTraffic(upload + download)} / {parseTraffic(total)}
                            </span>
                            <span title={t('shared.labels.expireTime') as string}>
                              {parseExpire(expire)}
                            </span>
                          </Box>
                          <LinearProgress
                            variant="determinate"
                            value={progress}
                            sx={{
                              height: 6,
                              borderRadius: 3,
                              opacity: total > 0 ? 1 : 0,
                            }}
                          />
                        </>
                      ) : null
                    }
                  />
                  <Divider orientation="vertical" flexItem />
                  <Box
                    sx={{
                      width: 40,
                      display: 'flex',
                      justifyContent: 'center',
                      alignItems: 'center',
                    }}
                  >
                    <IconButton
                      size="small"
                      color="primary"
                      onClick={() => void onUpdate(key)}
                      disabled={isUpdating}
                      sx={{
                        animation: isUpdating ? 'spin 1s linear infinite' : 'none',
                        '@keyframes spin': {
                          '0%': { transform: 'rotate(0deg)' },
                          '100%': { transform: 'rotate(360deg)' },
                        },
                      }}
                      title={t('proxies.page.provider.actions.update')}
                      aria-label={t('proxies.page.provider.actions.update')}
                    >
                      <RefreshRounded />
                    </IconButton>
                  </Box>
                </ListItem>
              )
            })}
          </List>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpen(false)} variant="outlined">
            {t('shared.actions.close')}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  )
}
