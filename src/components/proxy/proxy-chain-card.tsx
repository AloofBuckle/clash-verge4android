import { Delete as DeleteIcon, DragIndicator } from '@mui/icons-material'
import { Box, Chip, IconButton, Typography, useTheme } from '@mui/material'
import { type Ref } from 'react'
import { useTranslation } from 'react-i18next'

import type { ProxyChainItem } from './proxy-chain-model'

interface ProxyChainCardProps {
  proxy: ProxyChainItem
  index: number
  isFirst: boolean
  isLast: boolean
  isDragging?: boolean
  isDropping?: boolean
  handleRef?: Ref<HTMLElement> | null
  onRemove?: (id: string) => void
}

export const ProxyChainCard = ({
  proxy,
  index,
  isFirst,
  isLast,
  isDragging,
  isDropping,
  handleRef,
  onRemove,
}: ProxyChainCardProps) => {
  const theme = useTheme()
  const { t } = useTranslation()

  const roleLabel = isFirst
    ? t('proxies.page.chain.entryNode')
    : isLast
      ? t('proxies.page.chain.exitNode')
      : undefined

  const roleColor = isFirst
    ? theme.palette.success.main
    : isLast
      ? theme.palette.warning.main
      : undefined

  return (
    <Box
      sx={{
        mb: 0,
        display: 'flex',
        alignItems: 'center',
        p: 1,
        backgroundColor: theme.palette.background.default,
        borderRadius: 1,
        border: roleColor
          ? `1.5px solid ${roleColor}`
          : `1px solid ${theme.palette.divider}`,
        opacity: proxy.recordId === undefined ? 0.55 : undefined,
        transition: 'box-shadow 0.2s, background-color 0.2s',
        boxShadow: isDropping
          ? `0 0 0 2px ${theme.palette.primary.main}66`
          : undefined,
      }}
    >
      <Box
        ref={handleRef}
        sx={{
          display: 'flex',
          alignItems: 'center',
          mr: 1,
          color: theme.palette.text.secondary,
          cursor: isDragging ? 'grabbing' : 'grab',
        }}
      >
        <DragIndicator />
      </Box>

      {roleLabel ? (
        <Chip
          label={roleLabel}
          size="small"
          sx={{
            mr: 1,
            fontWeight: 700,
            color: '#fff',
            backgroundColor: roleColor,
          }}
        />
      ) : (
        <Chip
          label={`${index + 1}`}
          size="small"
          color="primary"
          sx={{ mr: 1, minWidth: 32 }}
        />
      )}

      <Typography
        variant="body2"
        sx={{
          flex: 1,
          fontWeight: 500,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}
      >
        {proxy.name}
      </Typography>

      {proxy.type && (
        <Chip
          label={proxy.type}
          size="small"
          variant="outlined"
          sx={{ mr: 1 }}
        />
      )}

      {proxy.delay !== undefined && (
        <Chip
          label={
            proxy.delay > 0 ? `${proxy.delay}ms` : t('shared.labels.timeout')
          }
          size="small"
          color={
            proxy.delay > 0 && proxy.delay < 200
              ? 'success'
              : proxy.delay > 0 && proxy.delay < 800
                ? 'warning'
                : 'error'
          }
          sx={{ mr: 1, fontSize: '0.7rem', minWidth: 50 }}
        />
      )}

      {onRemove && (
        <IconButton
          size="small"
          onClick={() => onRemove(proxy.id)}
          sx={{
            color: theme.palette.error.main,
            '&:hover': {
              backgroundColor: theme.palette.error.light + '20',
            },
          }}
        >
          <DeleteIcon fontSize="small" />
        </IconButton>
      )}
    </Box>
  )
}
