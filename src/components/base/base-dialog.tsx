import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  type SxProps,
  type Theme,
} from '@mui/material'
import { ReactNode, useRef } from 'react'

interface Props {
  title: ReactNode
  open: boolean
  okBtn?: ReactNode
  cancelBtn?: ReactNode
  disableEnforceFocus?: boolean
  disableOk?: boolean
  disableCancel?: boolean
  disableFooter?: boolean
  contentSx?: SxProps<Theme>
  children?: ReactNode
  loading?: boolean
  onOk?: () => void
  onCancel?: () => void
  onClose?: () => void
}

export interface DialogRef {
  open: () => void
  close: () => void
}

export const BaseDialog: React.FC<Props> = ({
  open,
  title,
  children,
  okBtn,
  cancelBtn,
  disableEnforceFocus,
  contentSx,
  disableCancel,
  disableOk,
  disableFooter,
  loading,
  onOk,
  onCancel,
  onClose,
}) => {
  // MUI leaves the dialog subtree mounted while the exit transition runs.
  // Callers often use the same nullable state as both the `open` flag and the
  // dialog payload, so closing it clears the payload one render before the
  // dialog disappears. Retain the last visible presentation while closing to
  // prevent fallback/empty UI from flashing during that transition.
  const retained = useRef({
    title,
    children,
    okBtn,
    cancelBtn,
    contentSx,
    disableCancel,
    disableOk,
    disableFooter,
    loading,
  })
  if (open) {
    retained.current = {
      title,
      children,
      okBtn,
      cancelBtn,
      contentSx,
      disableCancel,
      disableOk,
      disableFooter,
      loading,
    }
  }
  const visible = open
    ? {
        title,
        children,
        okBtn,
        cancelBtn,
        contentSx,
        disableCancel,
        disableOk,
        disableFooter,
        loading,
      }
    : retained.current

  return (
    <Dialog
      open={open}
      onClose={onClose}
      disableEnforceFocus={disableEnforceFocus}
    >
      <DialogTitle>{visible.title}</DialogTitle>

      <DialogContent sx={visible.contentSx}>{visible.children}</DialogContent>

      {!visible.disableFooter && (
        <DialogActions>
          {!visible.disableCancel && (
            <Button variant="outlined" onClick={onCancel}>
              {visible.cancelBtn}
            </Button>
          )}
          {!visible.disableOk && (
            <Button loading={visible.loading} variant="contained" onClick={onOk}>
              {visible.okBtn}
            </Button>
          )}
        </DialogActions>
      )}
    </Dialog>
  )
}
