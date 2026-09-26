import {
  ArrowDownwardRounded,
  ArrowUpwardRounded,
} from '@mui/icons-material'
import {
  alpha,
  Box,
  Button,
  ButtonGroup,
  Paper,
  Stack,
  Typography,
  useTheme,
} from '@mui/material'
import { type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseEmpty } from '@/components/base/base-empty'

export function MobileEmpty({ text, action }: { text: string; action?: ReactNode }) {
  return (
    <Box sx={{ minHeight: 260, width: '100%' }}>
      <BaseEmpty text={text} extra={action} />
    </Box>
  )
}

function TrafficStat({ icon, title, value, color }: { icon: ReactNode; title: string; value: string; color: 'primary' | 'secondary' }) {
  const theme = useTheme()
  const colorValue = theme.palette[color].main
  return (
    <Paper
      elevation={0}
      sx={{
        display: 'flex',
        alignItems: 'center',
        borderRadius: 2,
        bgcolor: alpha(colorValue, 0.05),
        border: `1px solid ${alpha(colorValue, 0.15)}`,
        p: 1,
        flex: 1,
        minWidth: 0,
      }}
    >
      <Box sx={{ mr: 1, ml: '2px', display: 'flex', alignItems: 'center', justifyContent: 'center', width: 32, height: 32, borderRadius: '50%', bgcolor: alpha(colorValue, 0.1), color: colorValue, flexShrink: 0 }}>
        {icon}
      </Box>
      <Box sx={{ minWidth: 0 }}>
        <Typography variant="caption" color="text.secondary" noWrap>{title}</Typography>
        <Typography variant="body1" noWrap sx={{ fontWeight: 'bold' }}>{value}</Typography>
      </Box>
    </Paper>
  )
}

export function MobileTrafficStats({ upload, download }: { upload: string; download: string }) {
  const { t } = useTranslation()
  return (
    <Stack direction="row" spacing={1}>
      <TrafficStat icon={<ArrowUpwardRounded fontSize="small" />} title={t('home.components.traffic.metrics.uploadSpeed')} value={upload} color="secondary" />
      <TrafficStat icon={<ArrowDownwardRounded fontSize="small" />} title={t('home.components.traffic.metrics.downloadSpeed')} value={download} color="primary" />
    </Stack>
  )
}

export function MobileModeButtons({
  mode,
  disabled,
  onChange,
}: {
  mode?: string
  disabled: boolean
  onChange: (mode: 'rule' | 'global' | 'direct') => void
}) {
  const { t } = useTranslation()
  return (
    <ButtonGroup size="small" fullWidth>
      {(['rule', 'global', 'direct'] as const).map((item) => (
        <Button
          key={item}
          variant={mode?.toLowerCase() === item ? 'contained' : 'outlined'}
          disabled={disabled}
          onClick={() => onChange(item)}
          sx={{ textTransform: 'capitalize' }}
        >
          {t(`proxies.page.modes.${item}`)}
        </Button>
      ))}
    </ButtonGroup>
  )
}
