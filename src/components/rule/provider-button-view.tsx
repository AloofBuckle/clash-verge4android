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

export interface RuleProviderButtonItem {
  name: string
  vehicleType: string
  behavior: string
  ruleCount: number
  updatedAt?: string | null
}

interface Props {
  providers: RuleProviderButtonItem[]
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

export const RuleProviderButtonView = ({
  providers,
  updating,
  onUpdate,
  onUpdateAll,
}: Props) => {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  if (providers.length === 0) return null

  return (
    <>
      <Button
        variant="outlined"
        size="small"
        startIcon={<StorageOutlined />}
        onClick={() => setOpen(true)}
      >
        {t('rules.page.provider.trigger')}
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
              {t('rules.page.provider.dialogTitle')}
            </Typography>
            <Button variant="contained" size="small" onClick={() => void onUpdateAll()}>
              {t('rules.page.provider.actions.updateAll')}
            </Button>
          </Box>
        </DialogTitle>
        <DialogContent>
          <List sx={{ py: 0, minHeight: 250 }}>
            {providers.map((provider) => {
              const isUpdating = updating[provider.name]
              const time = provider.updatedAt ? dayjs(provider.updatedAt) : null
              return (
                <ListItem
                  key={provider.name}
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
                        '&:hover': {
                          backgroundColor: hoverColor,
                          borderColor: alpha(primary.main, 0.3),
                        },
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
                          title={provider.name}
                          sx={{ display: 'flex', alignItems: 'center' }}
                        >
                          <span style={{ marginRight: '8px' }}>{provider.name}</span>
                          <TypeBox component="span">{provider.ruleCount}</TypeBox>
                        </Typography>
                        <Typography variant="body2" color="text.secondary" noWrap>
                          <small>{t('shared.labels.updateAt')}: </small>
                          {time?.fromNow() ?? '-'}
                        </Typography>
                      </Box>
                    }
                    secondary={
                      <Box sx={{ display: 'flex' }}>
                        <TypeBox component="span">{provider.vehicleType}</TypeBox>
                        <TypeBox component="span">{provider.behavior || '-'}</TypeBox>
                      </Box>
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
                      onClick={() => void onUpdate(provider.name)}
                      disabled={isUpdating}
                      aria-label={t('rules.page.provider.actions.update')}
                      sx={{
                        animation: isUpdating ? 'spin 1s linear infinite' : 'none',
                        '@keyframes spin': {
                          '0%': { transform: 'rotate(0deg)' },
                          '100%': { transform: 'rotate(360deg)' },
                        },
                      }}
                      title={t('rules.page.provider.actions.update')}
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
