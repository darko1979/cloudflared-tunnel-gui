import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, RefreshCw, Pencil, Trash2, Loader2, Circle } from 'lucide-react';
import dayjs from 'dayjs';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import { CreateTunnelDialog, UpdateTunnelDialog, DeleteTunnelDialog } from '@/components/tunnel-dialogs';
import type { CreateTunnelData } from '@/components/tunnel-dialogs';
import { useAppStore } from '@/store/app-store';
import type { Tunnel } from '@/types/api';
import { toast } from 'sonner';

export default function Tunnels() {
    const { t } = useTranslation();
    const [tunnels, setTunnels] = useState<Tunnel[]>([]);
    const [isListing, setIsListing] = useState(false);
    const [isBusy, setIsBusy] = useState(false);
    const [isCreateOpen, setIsCreateOpen] = useState(false);
    const [updating, setUpdating] = useState<Tunnel | null>(null);
    const [deleting, setDeleting] = useState<Tunnel | null>(null);

    const { tunnelStatus, setTunnelStatus, setConfig, addLog } = useAppStore();

    const loadTunnels = useCallback(async () => {
        if (!window.api) return;

        setIsListing(true);
        try {
            const result = await window.api.listTunnels();
            if (result.success) {
                setTunnels(result.tunnels);
            } else {
                toast.error(result.error || t('tunnels.listFailed'));
            }
        } catch {
            toast.error(t('tunnels.listFailed'));
        } finally {
            setIsListing(false);
        }
    }, [t]);

    useEffect(() => {
        loadTunnels();
        window.api?.getTunnelStatus().then(setTunnelStatus);
    }, [loadTunnels, setTunnelStatus]);

    // Bring the running tunnel in line with config.yml after a tunnel command
    const syncService = async (wasRunning: boolean, rawBefore: string) => {
        const configResult = await window.api.getConfig();
        if (configResult.success) {
            setConfig(configResult.config || null, configResult.raw || '', configResult.path);
        }
        const status = await window.api.getTunnelStatus();
        const configChanged = (configResult.raw || '') !== rawBefore;

        if (wasRunning && (configChanged || !status.running)) {
            if (configResult.config?.tunnel) {
                toast.info(t('tunnels.restarting'));
                const restartResult = await window.api.restartTunnel({ maxRetries: 3, retryDelay: 2000 });
                if (restartResult.success) {
                    setTunnelStatus({ running: true, pid: restartResult.pid || null });
                    toast.success(t('dashboard.running'));
                } else {
                    setTunnelStatus({ running: false, pid: null });
                    toast.error(restartResult.error || 'Failed to restart tunnel');
                }
            } else {
                await window.api.stopTunnel();
                setTunnelStatus({ running: false, pid: null });
                toast.info(t('tunnels.stoppedNoTunnel'));
            }
        } else {
            setTunnelStatus(status);
        }

        await loadTunnels();
    };

    // Run a tunnel command, then sync the service with the resulting config
    const runCommand = async (command: () => Promise<{ success: boolean; error?: string }>, successMessage: string) => {
        if (!window.api) return false;

        setIsBusy(true);
        try {
            const [status, configBefore] = await Promise.all([
                window.api.getTunnelStatus(),
                window.api.getConfig(),
            ]);
            const result = await command();
            if (result.success) {
                toast.success(successMessage);
                addLog(`[tunnels] ${successMessage}`);
            } else {
                toast.error(result.error || t('common.error'));
                addLog(`[tunnels] ${result.error}`);
            }
            await syncService(status.running, configBefore.raw || '');
            return result.success;
        } catch (error) {
            toast.error(String(error));
            return false;
        } finally {
            setIsBusy(false);
        }
    };

    const handleCreate = async (data: CreateTunnelData) => {
        const ok = await runCommand(
            async () => {
                const result = await window.api.createTunnel(data.name, data.makeActive, data.route);
                if (result.success && data.route) {
                    if (result.dnsError) {
                        toast.error(t('tunnels.dnsFailed', { hostname: data.route.hostname }));
                        addLog(`[dns] ${data.route.hostname}: failed - ${result.dnsError}`);
                    } else {
                        addLog(`[dns] ${data.route.hostname}: configured successfully`);
                    }
                }
                return result;
            },
            t('tunnels.created', { name: data.name }),
        );
        if (ok) setIsCreateOpen(false);
    };

    const handleUpdate = async (data: { makeActive: boolean; restoreCredentials: boolean }) => {
        if (!updating) return;
        const tunnel = updating;
        const ok = await runCommand(
            () => window.api.updateTunnel(tunnel.id, data),
            t('tunnels.updated', { name: tunnel.name }),
        );
        if (ok) setUpdating(null);
    };

    const handleDelete = async (force: boolean) => {
        if (!deleting) return;
        const tunnel = deleting;
        const ok = await runCommand(
            () => window.api.deleteTunnel(tunnel.id, force),
            t('tunnels.deleted', { name: tunnel.name }),
        );
        if (ok) setDeleting(null);
    };

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold">{t('tunnels.title')}</h1>
                    <p className="text-muted-foreground">{t('tunnels.description')}</p>
                </div>
                <Badge variant={tunnelStatus.running ? 'default' : 'secondary'} className="gap-1">
                    <Circle className={`h-2 w-2 fill-current ${tunnelStatus.running ? 'text-green-500' : 'text-gray-400'}`} />
                    {tunnelStatus.running ? t('dashboard.running') : t('dashboard.stopped')}
                </Badge>
            </div>

            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="text-lg">{t('tunnels.listTitle')}</CardTitle>
                            <CardDescription>{t('tunnels.listDescription')}</CardDescription>
                        </div>
                        <div className="flex gap-2">
                            <Button onClick={loadTunnels} disabled={isListing || isBusy} variant="outline" size="sm">
                                <RefreshCw className={`h-4 w-4 mr-2 ${isListing ? 'animate-spin' : ''}`} />
                                {t('dashboard.refresh')}
                            </Button>
                            <Button onClick={() => setIsCreateOpen(true)} disabled={isBusy} size="sm">
                                <Plus className="h-4 w-4 mr-2" />
                                {t('tunnels.create')}
                            </Button>
                        </div>
                    </div>
                </CardHeader>
                <CardContent>
                    {isListing && tunnels.length === 0 ? (
                        <div className="flex items-center justify-center py-12 text-muted-foreground">
                            <Loader2 className="h-5 w-5 mr-2 animate-spin" />
                            {t('common.loading')}
                        </div>
                    ) : tunnels.length === 0 ? (
                        <div className="flex flex-col items-center justify-center py-12 text-center">
                            <p className="text-muted-foreground">{t('tunnels.empty')}</p>
                            <p className="text-sm text-muted-foreground mt-1">{t('tunnels.emptyDescription')}</p>
                        </div>
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>{t('tunnels.name')}</TableHead>
                                    <TableHead>{t('tunnels.id')}</TableHead>
                                    <TableHead>{t('tunnels.createdAt')}</TableHead>
                                    <TableHead>{t('tunnels.connections')}</TableHead>
                                    <TableHead className="text-right">{t('table.actions')}</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {tunnels.map((tunnel) => {
                                    const connections = tunnel.connections || [];
                                    const colos = [...new Set(connections.map((c) => c.colo_name))];
                                    return (
                                        <TableRow key={tunnel.id}>
                                            <TableCell className="font-medium">
                                                <div className="flex flex-wrap items-center gap-2">
                                                    {tunnel.name}
                                                    {tunnel.isActive && <Badge>{t('tunnels.active')}</Badge>}
                                                    {!tunnel.hasCredentials && (
                                                        <Badge variant="outline" title={tunnel.credentialsFile}>
                                                            {t('tunnels.noCredentials')}
                                                        </Badge>
                                                    )}
                                                </div>
                                            </TableCell>
                                            <TableCell className="font-mono text-xs">{tunnel.id}</TableCell>
                                            <TableCell>{dayjs(tunnel.created_at).format('YYYY-MM-DD HH:mm')}</TableCell>
                                            <TableCell>
                                                <div className="flex items-center gap-1.5">
                                                    <Circle className={`h-2.5 w-2.5 ${connections.length > 0 ? 'text-green-500 fill-green-500' : 'text-muted-foreground fill-muted-foreground'}`} />
                                                    <span>{connections.length}</span>
                                                    {colos.length > 0 && (
                                                        <span className="text-xs text-muted-foreground">({colos.join(', ')})</span>
                                                    )}
                                                </div>
                                            </TableCell>
                                            <TableCell className="text-right">
                                                <div className="flex justify-end gap-1">
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        onClick={() => setUpdating(tunnel)}
                                                        disabled={isBusy}
                                                        title={t('tunnels.update')}
                                                    >
                                                        <Pencil className="h-4 w-4" />
                                                    </Button>
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        onClick={() => setDeleting(tunnel)}
                                                        disabled={isBusy}
                                                        title={t('table.delete')}
                                                        className="text-destructive hover:text-destructive"
                                                    >
                                                        <Trash2 className="h-4 w-4" />
                                                    </Button>
                                                </div>
                                            </TableCell>
                                        </TableRow>
                                    );
                                })}
                            </TableBody>
                        </Table>
                    )}
                </CardContent>
            </Card>

            <CreateTunnelDialog
                open={isCreateOpen}
                onOpenChange={setIsCreateOpen}
                onSubmit={handleCreate}
                isBusy={isBusy}
            />
            <UpdateTunnelDialog
                tunnel={updating}
                onOpenChange={(open) => !open && setUpdating(null)}
                onSubmit={handleUpdate}
                isBusy={isBusy}
            />
            <DeleteTunnelDialog
                tunnel={deleting}
                onOpenChange={(open) => !open && setDeleting(null)}
                onConfirm={handleDelete}
                isBusy={isBusy}
            />
        </div>
    );
}
